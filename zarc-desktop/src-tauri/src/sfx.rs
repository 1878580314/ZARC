use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::*;

const SFX_MAGIC: &[u8; 8] = b"ZARCSFX1";
const SFX_TRAILER_LEN: u64 = 24;
const SIDECAR_SUFFIX: &str = ".payload";
/// 超过此载荷大小，SFX 采用外部 sidecar 布局；低于它则把载荷内嵌进 EXE（单文件、更便携）。
/// Windows PE 加载器拒绝约 2GB 以上的单映像，故 1.9GB 为宿主 EXE + manifest + trailer 留出余量。
/// Above this payload size, SFX uses the external sidecar layout; below it,
/// payload is embedded in the exe (single-file, more portable). The Windows PE
/// loader rejects single images larger than ~2GB, so 1.9GB leaves headroom for
/// the host exe + manifest + trailer.
const SFX_SIDECAR_THRESHOLD: u64 = 1_900_000_000;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SfxManifest {
    payload_offset: u64,
    payload_length: u64,
    encrypted: bool,
    archive_kind: ArchiveKind,
    default_extract_name: String,
    source_name: String,
    created_by_version: String,
    /// 为 true 时载荷位于 `<exe>.payload` sidecar（超 2GB 容量后的布局）；
    /// 为 false（或旧版 SFX 中缺省）时载荷内嵌在 EXE 自身中。
    /// When true, payload lives in `<exe>.payload` sidecar (post-2GB-capacity layout).
    /// When false (or absent in legacy SFX), payload is embedded in the exe itself.
    #[serde(default)]
    payload_in_sidecar: bool,
}

pub(super) fn load_embedded_archive_info_from_current_exe() -> Result<Option<EmbeddedArchiveInfo>> {
    let host = std::env::current_exe().context("无法定位当前程序")?;
    load_embedded_archive_info_from_path(&host)
}

pub(super) fn extract_embedded_archive_from_current_exe(
    request: EmbeddedDecompressRequest,
    app: Option<AppHandle>,
    state: Option<AppState>,
) -> Result<OperationReport> {
    let host = std::env::current_exe().context("无法定位当前程序")?;
    extract_embedded_archive_from_path(&host, request, app, state)
}

pub(super) fn compress_sfx_archive_sync(
    request: CompressRequest,
    output: PathBuf,
    enable_logging: bool,
    delete_source_after: bool,
    reporter: ProgressReporter,
    state: Option<AppState>,
    source_bytes: u64,
) -> Result<OperationReport> {
    if !cfg!(target_os = "windows") {
        let err = anyhow!("Windows 自解压 EXE 仅能在 Windows 构建环境中生成");
        reporter.fail(full_error_chain(&err));
        return Err(err);
    }

    let source = PathBuf::from(request.source_path.trim());
    let password = normalize_password(request.password.clone());
    let host_exe = std::env::current_exe().context("无法定位当前程序")?;
    if output == host_exe {
        let err = anyhow!("输出路径不能覆盖当前运行中的程序");
        reporter.fail(full_error_chain(&err));
        return Err(err);
    }

    log_to_file(
        enable_logging,
        &format!(
            "开始生成 Windows 自解压包: {} -> {}",
            source.display(),
            output.display()
        ),
    );

    let started = Instant::now();
    let sfx_source = SfxSource {
        path: &source,
        level: request.level.unwrap_or(8).clamp(1, 22),
        include_root_dir: request.include_root_dir.unwrap_or(true),
        password: password.as_deref(),
        reporter: &reporter,
        state: state.as_ref(),
        threads: request.threads,
        source_bytes,
    };
    let outcome = match build_sfx_streamed(&host_exe, &sfx_source, &output, SFX_SIDECAR_THRESHOLD) {
        Ok(outcome) => outcome,
        Err(err) => {
            cleanup_sfx_output(&output);
            reporter.fail(full_error_chain(&err));
            log_to_file(enable_logging, &format!("生成自解压包失败: {err:#}"));
            return Err(err);
        }
    };

    let duration = started.elapsed().as_secs_f64();

    log_to_file(
        enable_logging,
        &format!(
            "生成自解压包完成. 原始大小: {}, 输出大小: {}, 耗时: {:.2}s",
            source_bytes, outcome.output_bytes, duration
        ),
    );

    check_abort(state.as_ref())?;
    if delete_source_after {
        reporter.phase("cleanup");
        maybe_delete_source(&source, enable_logging, &reporter)?;
    }

    reporter.finish();
    Ok(OperationReport {
        operation: "compress".to_string(),
        source_path: path_to_string(&source),
        output_path: path_to_string(&output),
        source_bytes,
        output_bytes: outcome.output_bytes,
        duration_ms: duration * 1000.0,
        throughput_mi_bs: throughput(source_bytes, duration),
        compression_ratio: Some(ratio(outcome.output_bytes, source_bytes)),
        blake3_hash: Some(outcome.hash),
        sidecar_path: outcome.sidecar.map(|p| path_to_string(&p)),
    })
}

/// 待压缩源及其压缩、进度、取消上下文
/// The source being compressed plus its compression, progress and abort context.
struct SfxSource<'a> {
    path: &'a Path,
    level: i32,
    include_root_dir: bool,
    password: Option<&'a str>,
    reporter: &'a ProgressReporter,
    state: Option<&'a AppState>,
    threads: Option<u32>,
    source_bytes: u64,
}

/// SFX 构建结果：报告所需的落盘产物信息
/// Result of an SFX build: everything the operation report needs.
struct SfxBuildOutcome {
    /// sidecar 布局时为 `.payload` 文件路径；嵌入式为 None
    /// Path of the `.payload` sidecar in sidecar layout; None when embedded.
    sidecar: Option<PathBuf>,
    hash: String,
    output_bytes: u64,
}

/// 输出目录中的 SFX 暂存文件：所有写入经 BLAKE3 哈希与缓冲包装，成功后原子 persist。
/// Staging file for SFX output beside the target: every byte written passes
/// through the BLAKE3 hasher and a buffer, then the file is atomically
/// persisted into place.
struct SfxStaging {
    temp: tempfile::NamedTempFile,
    hash: Arc<Mutex<blake3::Hasher>>,
}

impl SfxStaging {
    fn new_in(parent: &Path) -> Result<Self> {
        Ok(Self {
            temp: tempfile::NamedTempFile::new_in(parent)
                .with_context(|| format!("无法在输出目录创建临时文件: {}", parent.display()))?,
            hash: Arc::new(Mutex::new(blake3::Hasher::new())),
        })
    }

    /// 每个写阶段取一个新缓冲写回器；落盘位置沿用文件当前偏移
    /// Each write phase takes a fresh buffered writer; bytes land at the
    /// file's current offset.
    fn writer(&mut self) -> BufWriter<HashingWriter<&mut File>> {
        BufWriter::with_capacity(
            IO_BUFFER_SIZE,
            HashingWriter::new(self.temp.as_file_mut(), self.hash.clone()),
        )
    }

    /// 复制输入流进暂存并返回字节数 / Copy a reader into staging, returning bytes copied.
    fn copy_in<R: Read>(&mut self, reader: &mut R) -> Result<u64> {
        let mut writer = self.writer();
        let copied = io::copy(reader, &mut writer).context("复制自解压载荷失败")?;
        writer.flush().context("刷新自解压输出失败")?;
        Ok(copied)
    }

    /// 把压缩载荷直写进暂存（可选加密）；zstd 流排空即返回
    /// Stream the compressed payload (optionally encrypted) straight into
    /// staging; returns once the zstd workers drain.
    fn write_payload(&mut self, is_dir: bool, source: &SfxSource) -> Result<()> {
        let job = CompressJob {
            level: source.level,
            include_root_dir: source.include_root_dir,
            source_bytes: source.source_bytes,
            reporter: source.reporter,
            state: source.state,
            threads: source.threads,
        };
        let writer = self.writer();
        let mut writer = match source.password {
            Some(pwd) => {
                let enc = EncryptedWriter::new(writer, pwd)?;
                let enc = if is_dir {
                    compress_directory(source.path, enc, &job)?
                } else {
                    compress_file(source.path, enc, &job)?
                };
                enc.finish().context("完成加密输出失败")?
            }
            None => {
                if is_dir {
                    compress_directory(source.path, writer, &job)?
                } else {
                    compress_file(source.path, writer, &job)?
                }
            }
        };
        writer.flush().context("刷新自解压输出失败")
    }

    /// 追加 `[manifest][magic][manifest_offset][manifest_len]` 尾部
    /// Append the `[manifest][magic][manifest_offset][manifest_len]` trailer.
    fn append_manifest_and_trailer(
        &mut self,
        manifest_bytes: &[u8],
        manifest_offset: u64,
    ) -> Result<()> {
        let mut writer = self.writer();
        writer
            .write_all(manifest_bytes)
            .context("写入 SFX manifest 失败")?;
        writer
            .write_all(SFX_MAGIC)
            .context("写入 SFX trailer magic 失败")?;
        writer
            .write_all(&manifest_offset.to_le_bytes())
            .context("写入 SFX trailer manifest offset 失败")?;
        writer
            .write_all(&(manifest_bytes.len() as u64).to_le_bytes())
            .context("写入 SFX trailer manifest length 失败")?;
        writer.flush().context("刷新 SFX 输出失败")
    }

    fn len(&self) -> Result<u64> {
        Ok(self.temp.as_file().metadata()?.len())
    }

    /// 截短暂存并把写指针收回 `len`；用于把“宿主+载荷”改写成“宿主+清单”
    /// Truncate staging and rewind the write position to `len`; used to turn
    /// a `[host][payload]` staging back into `[host][manifest]`.
    fn truncate(&mut self, len: u64) -> Result<()> {
        self.temp
            .as_file()
            .set_len(len)
            .context("截断自解压暂存文件失败")?;
        self.temp
            .as_file_mut()
            .seek(SeekFrom::Start(len))
            .context("定位自解压暂存文件失败")?;
        Ok(())
    }

    fn sync(&self) -> Result<()> {
        self.temp.as_file().sync_all().context("同步自解压文件失败")
    }

    fn digest(&self) -> String {
        hash_digest(&self.hash)
    }

    fn persist(self, target: &Path) -> Result<()> {
        self.temp
            .persist_noclobber(target)
            .map_err(|err| err.error)
            .with_context(|| format!("保存 SFX 输出失败: {}", target.display()))?;
        Ok(())
    }
}

fn sfx_manifest_bytes(
    payload_offset: u64,
    payload_length: u64,
    encrypted: bool,
    archive_kind: ArchiveKind,
    name: &str,
    payload_in_sidecar: bool,
) -> Result<Vec<u8>> {
    let manifest = SfxManifest {
        payload_offset,
        payload_length,
        encrypted,
        archive_kind,
        default_extract_name: name.to_string(),
        source_name: name.to_string(),
        created_by_version: env!("CARGO_PKG_VERSION").to_string(),
        payload_in_sidecar,
    };
    serde_json::to_vec(&manifest).context("无法序列化 SFX manifest")
}

/// 写 sidecar 布局的宿主 EXE：`[host][manifest][trailer]`，载荷在旁文件中。
/// Write the sidecar-layout host exe: `[host][manifest][trailer]`; the payload
/// lives in the sibling sidecar file.
fn write_sfx_host_manifest(
    host: &Path,
    output: &Path,
    manifest_bytes: &[u8],
    state: Option<&AppState>,
) -> Result<()> {
    let mut staging = SfxStaging::new_in(output_parent(output))?;
    {
        let mut host_file =
            File::open(host).with_context(|| format!("无法打开宿主程序: {}", host.display()))?;
        let mut reader = AbortableReader::new(&mut host_file, state);
        staging
            .copy_in(&mut reader)
            .with_context(|| format!("复制宿主程序失败: {}", host.display()))?;
    }
    let host_len = staging.len()?;
    staging.append_manifest_and_trailer(manifest_bytes, host_len)?;
    staging.sync()?;
    staging.persist(output)
}

/// 压缩载荷直写最终容器，不经中间归档文件。
///
/// 布局由载荷实际大小决定，但写盘前未知——按源大小预判：源未达阈值则按嵌入式
/// `[host][payload]` 直写，达到阈值则按 sidecar 直写载荷。预判错误时回退为一次
/// 载荷拷贝，与旧实现成本相同；预判正确则省掉一整趟载荷读写。
/// Stream the compressed payload straight into the final container — no
/// intermediate archive file.
///
/// The layout depends on the real payload size, unknown until compression
/// finishes, so it is predicted from the source size: a source below the
/// threshold streams `[host][payload]` directly, one at or above it streams
/// the sidecar payload directly. A mispredict falls back to a single payload
/// copy — the same cost as the previous always-copy implementation, while a
/// correct prediction eliminates a whole payload read+write pass.
fn build_sfx_streamed(
    host_exe: &Path,
    source: &SfxSource,
    output_exe: &Path,
    sidecar_threshold: u64,
) -> Result<SfxBuildOutcome> {
    let source_bytes = source.source_bytes;
    let source_access = fs_access_path(source.path)
        .with_context(|| format!("无法准备源路径: {}", source.path.display()))?;
    let is_dir = fs::metadata(&source_access)
        .with_context(|| format!("源路径不存在或无法访问: {}", source.path.display()))?
        .is_dir();
    let kind = if is_dir {
        ArchiveKind::TarZst
    } else {
        ArchiveKind::Zst
    };
    let encrypted = source.password.is_some();
    let parent = output_parent(output_exe);
    let reporter = source.reporter;
    let state = source.state;
    let source_name = source
        .path
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());

    if source_bytes >= sidecar_threshold {
        let mut staging = SfxStaging::new_in(parent)?;
        staging.write_payload(is_dir, source)?;
        let payload_length = staging.len()?;

        if payload_length >= sidecar_threshold {
            reporter.phase("packaging");
            let manifest =
                sfx_manifest_bytes(0, payload_length, encrypted, kind, &source_name, true)?;
            staging.append_manifest_and_trailer(&manifest, payload_length)?;
            staging.sync()?;
            check_abort(state)?;
            let digest = staging.digest();
            let sidecar = sidecar_path(output_exe);
            staging.persist(&sidecar)?;
            write_sfx_host_manifest(host_exe, output_exe, &manifest, state)?;
            return Ok(SfxBuildOutcome {
                output_bytes: fs::metadata(&sidecar)
                    .with_context(|| format!("无法读取结果文件信息: {}", sidecar.display()))?
                    .len(),
                sidecar: Some(sidecar),
                hash: digest,
            });
        }

        // 载荷意外小于阈值：并入嵌入式 EXE（一次载荷拷贝）
        // Payload unexpectedly small: fold it into an embedded exe (one copy).
        reporter.phase("packaging");
        let mut exe = SfxStaging::new_in(parent)?;
        let host_len = {
            let mut host = File::open(host_exe)
                .with_context(|| format!("无法打开宿主程序: {}", host_exe.display()))?;
            exe.copy_in(&mut host)
                .with_context(|| format!("复制宿主程序失败: {}", host_exe.display()))?;
            exe.len()?
        };
        {
            let payload_in = File::open(staging.temp.path()).context("无法回读自解压载荷")?;
            let mut reader = AbortableReader::new(payload_in.take(payload_length), state);
            let copied = exe.copy_in(&mut reader)?;
            if copied != payload_length {
                bail!("自解压载荷被截断");
            }
        }
        let manifest = sfx_manifest_bytes(
            host_len,
            payload_length,
            encrypted,
            kind,
            &source_name,
            false,
        )?;
        exe.append_manifest_and_trailer(&manifest, host_len + payload_length)?;
        exe.sync()?;
        check_abort(state)?;
        let digest = exe.digest();
        exe.persist(output_exe)?;
        return Ok(SfxBuildOutcome {
            output_bytes: fs::metadata(output_exe)
                .with_context(|| format!("无法读取结果文件信息: {}", output_exe.display()))?
                .len(),
            sidecar: None,
            hash: digest,
        });
    }

    let mut staging = SfxStaging::new_in(parent)?;
    let host_len = {
        let mut host = File::open(host_exe)
            .with_context(|| format!("无法打开宿主程序: {}", host_exe.display()))?;
        staging
            .copy_in(&mut host)
            .with_context(|| format!("复制宿主程序失败: {}", host_exe.display()))?;
        staging.len()?
    };
    staging.write_payload(is_dir, source)?;
    let payload_length = staging.len()?.saturating_sub(host_len);

    if payload_length < sidecar_threshold {
        reporter.phase("packaging");
        let manifest = sfx_manifest_bytes(
            host_len,
            payload_length,
            encrypted,
            kind,
            &source_name,
            false,
        )?;
        staging.append_manifest_and_trailer(&manifest, host_len + payload_length)?;
        staging.sync()?;
        check_abort(state)?;
        let digest = staging.digest();
        staging.persist(output_exe)?;
        return Ok(SfxBuildOutcome {
            output_bytes: fs::metadata(output_exe)
                .with_context(|| format!("无法读取结果文件信息: {}", output_exe.display()))?
                .len(),
            sidecar: None,
            hash: digest,
        });
    }

    // 载荷意外越界：拆出载荷段写入 sidecar，暂存截短为宿主+清单 EXE
    // Payload overshot the cap: copy it out into a sidecar, then truncate the
    // staging file back down to host + manifest.
    reporter.phase("packaging");
    let manifest = sfx_manifest_bytes(0, payload_length, encrypted, kind, &source_name, true)?;
    let sidecar = sidecar_path(output_exe);
    let mut sidecar_staging = SfxStaging::new_in(parent)?;
    {
        let mut payload_in = File::open(staging.temp.path()).context("无法回读自解压载荷")?;
        payload_in
            .seek(SeekFrom::Start(host_len))
            .context("定位自解压载荷失败")?;
        let mut reader = AbortableReader::new(payload_in.take(payload_length), state);
        let copied = sidecar_staging.copy_in(&mut reader)?;
        if copied != payload_length {
            bail!("自解压载荷被截断");
        }
    }
    sidecar_staging.append_manifest_and_trailer(&manifest, payload_length)?;
    sidecar_staging.sync()?;
    let digest = sidecar_staging.digest();
    sidecar_staging.persist(&sidecar)?;

    staging.truncate(host_len)?;
    staging.append_manifest_and_trailer(&manifest, host_len)?;
    staging.sync()?;
    check_abort(state)?;
    staging.persist(output_exe)?;
    Ok(SfxBuildOutcome {
        output_bytes: fs::metadata(&sidecar)
            .with_context(|| format!("无法读取结果文件信息: {}", sidecar.display()))?
            .len(),
        sidecar: Some(sidecar),
        hash: digest,
    })
}

/// 原子写入 `[host?][payload][manifest][trailer]` 容器：嵌入式 SFX 先拷宿主，
/// sidecar 只写载荷。
/// Atomically write a `[host?][payload][manifest][trailer]` container: embedded
/// SFX copies the host first, sidecar writes payload only.
#[cfg(test)]
fn write_sfx_container(
    target: &Path,
    host_exe: Option<&Path>,
    archive_path: &Path,
    payload_length: u64,
    manifest_bytes: &[u8],
    manifest_offset: u64,
    parent: &Path,
) -> Result<()> {
    write_sfx_container_with_state(
        target,
        host_exe,
        archive_path,
        payload_length,
        manifest_bytes,
        manifest_offset,
        parent,
        None,
    )
}

#[cfg(test)]
fn write_sidecar_host(
    host: &Path,
    output: &Path,
    manifest: &[u8],
    state: Option<&AppState>,
) -> Result<()> {
    write_sfx_container_with_state(
        output,
        Some(host),
        host,
        0,
        manifest,
        fs::metadata(host)?.len(),
        output_parent(output),
        state,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn write_sfx_container_with_state(
    target: &Path,
    host_exe: Option<&Path>,
    archive_path: &Path,
    payload_length: u64,
    manifest_bytes: &[u8],
    manifest_offset: u64,
    parent: &Path,
    state: Option<&AppState>,
) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("无法在输出目录创建临时文件: {}", parent.display()))?;
    {
        let mut writer = BufWriter::with_capacity(IO_BUFFER_SIZE, temp.as_file_mut());
        if let Some(host) = host_exe {
            let mut host_file = File::open(host)
                .with_context(|| format!("无法打开宿主程序: {}", host.display()))?;
            io::copy(&mut host_file, &mut writer)
                .with_context(|| format!("复制宿主程序失败: {}", host.display()))?;
        }
        let file = File::open(archive_path)?;
        let mut reader = AbortableReader::new(file.take(payload_length), state);
        let copied = io::copy(&mut reader, &mut writer).context("复制自解压载荷失败")?;
        if copied != payload_length {
            bail!("自解压载荷被截断");
        }
        writer
            .write_all(manifest_bytes)
            .context("写入 SFX manifest 失败")?;
        writer
            .write_all(SFX_MAGIC)
            .context("写入 SFX trailer magic 失败")?;
        writer
            .write_all(&manifest_offset.to_le_bytes())
            .context("写入 SFX trailer manifest offset 失败")?;
        writer
            .write_all(&(manifest_bytes.len() as u64).to_le_bytes())
            .context("写入 SFX trailer manifest length 失败")?;
        writer.flush().context("刷新 SFX 输出失败")?;
    }
    temp.as_file().sync_all().context("同步自解压文件失败")?;
    temp.persist_noclobber(target)
        .map_err(|err| err.error)
        .with_context(|| format!("保存 SFX 输出失败: {}", target.display()))?;
    Ok(())
}

/// 伴随 SFX EXE 的载荷 sidecar 路径。/ Path of the payload sidecar that accompanies an SFX exe.
pub(super) fn sidecar_path(exe: &Path) -> PathBuf {
    let mut os_string = exe.as_os_str().to_owned();
    os_string.push(SIDECAR_SUFFIX);
    PathBuf::from(os_string)
}

/// 尽力删除 SFX EXE 及其 sidecar（失败清理用）。/ Remove both the SFX exe and its sidecar (best-effort, for failure cleanup).
fn cleanup_sfx_output(exe: &Path) {
    let _ = fs::remove_file(exe);
    let _ = fs::remove_file(sidecar_path(exe));
}

fn load_embedded_archive_info_from_path(path: &Path) -> Result<Option<EmbeddedArchiveInfo>> {
    let Some(manifest) = read_embedded_manifest(path)? else {
        return Ok(None);
    };
    Ok(Some(EmbeddedArchiveInfo {
        host_path: path_to_string(path),
        payload_bytes: manifest.payload_length,
        default_extract_name: manifest.default_extract_name,
        encrypted: manifest.encrypted,
        archive_kind: archive_kind_label(manifest.archive_kind),
        // 与解压路径的检查保持同一语义：分卷模式要求 sidecar 与 EXE 同目录同名。
        // Same rule as the decompress-path check: split mode needs the sidecar next to the exe, same name.
        payload_ready: !manifest.payload_in_sidecar || sidecar_path(path).exists(),
    }))
}

fn extract_embedded_archive_from_path(
    host_path: &Path,
    request: EmbeddedDecompressRequest,
    app: Option<AppHandle>,
    state: Option<AppState>,
) -> Result<OperationReport> {
    let manifest = read_embedded_manifest(host_path)?
        .with_context(|| format!("文件未包含嵌入归档: {}", host_path.display()))?;
    let password = normalize_password(request.password);
    if manifest.encrypted && password.is_none() {
        bail!("该自解压包已加密，请输入解密密码");
    }

    let output_root = request
        .output_path
        .map(|value| PathBuf::from(value.trim()))
        .filter(|value| !value.as_os_str().is_empty())
        .with_context(|| "请选择解压目标目录")?;
    fs::create_dir_all(&output_root)
        .with_context(|| format!("无法创建解压目录: {}", output_root.display()))?;
    // 名称来自归档，即来自构建 SFX 的一方。原样拼接会让 `../../evil` 或 `C:/Windows/x`
    // 逃出用户选择的目录。
    // The name comes from the archive, i.e. from whoever built the SFX. Joining
    // it raw let `../../evil` or `C:/Windows/x` escape the chosen directory.
    let output = output_root.join(sanitize_extract_name(&manifest.default_extract_name)?);

    let payload_path = if manifest.payload_in_sidecar {
        let sidecar = sidecar_path(host_path);
        if !sidecar.exists() {
            bail!(
                "找不到数据文件 {}：请将它与本程序放在同一目录，且不要更改其名称",
                sidecar.display()
            );
        }
        sidecar
    } else {
        host_path.to_path_buf()
    };

    let source_bytes = manifest.payload_length;

    let reporter = ProgressReporter::new(app, "decompress", manifest.payload_length);
    reporter.begin();
    let started = Instant::now();

    let mut file = File::open(&payload_path)
        .with_context(|| format!("无法打开自解压数据: {}", payload_path.display()))?;
    file.seek(SeekFrom::Start(manifest.payload_offset))
        .with_context(|| "无法定位嵌入归档数据")?;
    let section_reader = file.take(manifest.payload_length);
    // 与普通解压一致：摘要随数据流同步计算，载荷即归档本体
    // Same as the plain decompress path: the digest rides along with the
    // data stream — the payload IS the archive body.
    let hash = Arc::new(Mutex::new(blake3::Hasher::new()));
    let hashing_reader = HashingReader {
        inner: section_reader,
        hash: hash.clone(),
    };
    let buf_reader = BufReader::with_capacity(IO_BUFFER_SIZE, hashing_reader);
    let progress_reader = ProgressReader::new(buf_reader, reporter.clone());

    // 委托给共享的事务性路径：先暂存到临时同级路径，仅在成功后重命名落位；
    // 中途失败不会留下残缺目录树，也不会碰任何已存在的同名文件。
    // Delegated to the shared transactional path, which stages into a temp
    // sibling and only renames into place on success; a mid-extraction failure
    // leaves no partial tree and never touches a pre-existing same-name file.
    let output_result = if manifest.encrypted {
        match EncryptedReader::new(progress_reader, password.as_deref().unwrap_or_default()) {
            Ok(decrypt_reader) => decompress_reader_transactionally(
                decrypt_reader,
                manifest.archive_kind,
                &output,
                state.as_ref(),
            ),
            Err(err) => Err(err),
        }
    } else {
        decompress_reader_transactionally(
            progress_reader,
            manifest.archive_kind,
            &output,
            state.as_ref(),
        )
    };

    let output_bytes = match output_result {
        Ok(bytes) => bytes,
        Err(err) => {
            reporter.fail(full_error_chain(&err));
            return Err(err);
        }
    };

    let hash = Some(hash_digest(&hash));
    let duration = started.elapsed().as_secs_f64();
    reporter.finish();

    Ok(OperationReport {
        operation: "decompress".to_string(),
        source_path: path_to_string(host_path),
        output_path: path_to_string(&output),
        source_bytes,
        output_bytes,
        duration_ms: duration * 1000.0,
        throughput_mi_bs: throughput(output_bytes.max(manifest.payload_length), duration),
        compression_ratio: None,
        blake3_hash: hash,
        sidecar_path: None,
    })
}

/// 把归档提供的解压名收敛为单个安全的路径分量。
///
/// `default_extract_name` 是从 SFX manifest 读出的攻击者可控数据。`output_root.join(raw)`
/// 会遵从绝对路径与 `..`，因此精心构造的 SFX 可写到用户能写的任意位置——逃出所选目录之外。
/// 只保留最后一个分量即可消除这两类逃逸。
///
/// Reduce an archive-supplied extraction name to a single, safe path component.
///
/// `default_extract_name` is attacker-controlled data read out of the SFX
/// manifest. `output_root.join(raw)` honours absolute paths and `..`, so a
/// crafted SFX could write anywhere the user could — outside the directory they
/// picked. Keeping only the final component removes both escapes.
fn sanitize_extract_name(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("自解压包内的输出名称为空，无法解压");
    }

    // 无论宿主 OS 如何，两种分隔符都被拒绝：Windows 构建的 SFX 在 Linux 上打开时不得越界，反之亦然。
    // Both separators are rejected regardless of host OS: a Windows-built SFX
    // must not be able to traverse when opened on Linux, and vice versa.
    let last = trimmed
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or_default();

    let name = Path::new(last)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    if name.is_empty() || name == "." || name == ".." {
        bail!("自解压包内的输出名称非法: {raw}");
    }
    Ok(name)
}

fn read_embedded_manifest(exe_path: &Path) -> Result<Option<SfxManifest>> {
    let sidecar = sidecar_path(exe_path);
    if sidecar.exists() {
        // sidecar 存在：任何损坏都是硬错误（不要静默回退到普通 ZARC 模式——用户显然想要 SFX）。
        // Sidecar present: any corruption is a hard error (don't silently fall back
        // to plain ZARC mode — the user clearly intended an SFX here).
        return read_manifest_from_file(&sidecar, true);
    }
    // 无 sidecar：尝试旧版嵌入式布局（trailer 在 EXE 末尾）；没有 trailer 说明这只是普通 ZARC EXE。
    // No sidecar: try legacy embedded layout (trailer at exe end); absent trailer
    // means this is just a normal ZARC exe.
    read_manifest_from_file(exe_path, false)
}

fn read_manifest_from_file(path: &Path, strict: bool) -> Result<Option<SfxManifest>> {
    let metadata =
        fs::metadata(path).with_context(|| format!("无法读取文件信息: {}", path.display()))?;
    if metadata.len() < SFX_TRAILER_LEN {
        if strict {
            bail!("数据文件已损坏（过短）: {}", path.display());
        }
        return Ok(None);
    }

    let mut file = File::open(path).with_context(|| format!("无法打开文件: {}", path.display()))?;
    file.seek(SeekFrom::End(-(SFX_TRAILER_LEN as i64)))
        .with_context(|| "无法定位 SFX trailer")?;

    let mut magic = [0_u8; SFX_MAGIC.len()];
    file.read_exact(&mut magic)
        .context("无法读取 SFX trailer magic")?;
    if &magic != SFX_MAGIC {
        if strict {
            bail!("数据文件已损坏（标识不符）: {}", path.display());
        }
        return Ok(None);
    }

    let mut offset_buf = [0_u8; 8];
    let mut len_buf = [0_u8; 8];
    file.read_exact(&mut offset_buf)
        .context("无法读取 SFX trailer manifest offset")?;
    file.read_exact(&mut len_buf)
        .context("无法读取 SFX trailer manifest length")?;

    let manifest_offset = u64::from_le_bytes(offset_buf);
    let manifest_length = u64::from_le_bytes(len_buf);
    let trailer_start = metadata.len() - SFX_TRAILER_LEN;
    let manifest_end = manifest_offset
        .checked_add(manifest_length)
        .with_context(|| "SFX manifest 长度非法")?;
    if manifest_offset > trailer_start || manifest_end > trailer_start {
        bail!("SFX manifest 超出有效范围");
    }

    file.seek(SeekFrom::Start(manifest_offset))
        .context("无法定位 SFX manifest")?;
    let manifest_len: usize = manifest_length
        .try_into()
        .with_context(|| "SFX manifest 过大")?;
    if manifest_len > 64 * 1024 {
        bail!("SFX manifest 超出 64 KiB 上限");
    }
    let mut manifest_bytes = vec![0_u8; manifest_len];
    file.read_exact(&mut manifest_bytes)
        .context("无法读取 SFX manifest")?;
    let manifest: SfxManifest =
        serde_json::from_slice(&manifest_bytes).context("无法解析 SFX manifest")?;
    let payload_end = manifest
        .payload_offset
        .checked_add(manifest.payload_length)
        .with_context(|| "SFX payload 长度非法")?;
    if (strict || !manifest.payload_in_sidecar) && payload_end > manifest_offset {
        bail!("SFX payload 超出有效范围");
    }
    Ok(Some(manifest))
}

fn archive_kind_label(kind: ArchiveKind) -> String {
    match kind {
        ArchiveKind::TarZst => "tar.zst".to_string(),
        ArchiveKind::Zst => "zst".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用：把源压缩为归档文件 / Test helper: compress a source into an archive file.
    fn compress_to_archive(source: &Path, archive: &Path, password: Option<&str>) {
        let reporter = ProgressReporter::new(None, "compress", fs::metadata(source).unwrap().len());
        let hash = Arc::new(Mutex::new(blake3::Hasher::new()));
        let sink = create_output_sink(archive, password, None, hash).expect("output sink");
        let job = CompressJob {
            level: 8,
            include_root_dir: true,
            source_bytes: 0,
            reporter: &reporter,
            state: None,
            threads: Some(1),
        };
        let sink = if source.is_dir() {
            compress_directory(source, sink, &job).expect("compress")
        } else {
            compress_file(source, sink, &job).expect("compress")
        };
        sink.finalize().expect("finalize");
    }

    /// 测试用：经生产直写路径构建 SFX；`threshold` 操控布局预判
    /// （0 强制 sidecar，`u64::MAX` 强制嵌入式）。
    /// Test helper: build an SFX through the production streaming path;
    /// `threshold` steers the layout prediction (0 forces sidecar,
    /// `u64::MAX` forces embedded).
    fn build_sfx(
        temp: &Path,
        source: &Path,
        password: Option<&str>,
        threshold: u64,
        output_name: &str,
    ) -> PathBuf {
        let template = temp.join("template.exe");
        if !template.exists() {
            fs::write(&template, b"MZfake-host").expect("write template");
        }
        let output = temp.join(output_name);
        let source_bytes = fs::metadata(source).unwrap().len();
        let reporter = ProgressReporter::new(None, "compress", source_bytes);
        let sfx_source = SfxSource {
            path: source,
            level: 8,
            include_root_dir: true,
            password,
            reporter: &reporter,
            state: None,
            threads: Some(1),
            source_bytes,
        };
        let outcome =
            build_sfx_streamed(&template, &sfx_source, &output, threshold).expect("build sfx");
        if threshold == 0 {
            assert!(outcome.sidecar.is_some(), "expected forced sidecar layout");
        }
        output
    }

    /// 通过生产路径从小源构建 SFX。返回 EXE 路径；小载荷采用嵌入式（单文件）布局。
    /// Build an SFX from a small source via the production path. Returns the exe
    /// path; small payloads take the embedded (single-file) layout.
    fn build_small_sfx(temp: &Path, source_name: &str, password: Option<&str>) -> PathBuf {
        let source = temp.join(source_name);
        fs::write(&source, b"hello from sfx").expect("write source");
        let output = build_sfx(temp, &source, password, u64::MAX, "plain.sfx.exe");
        assert!(
            !sidecar_path(&output).exists(),
            "no sidecar for embedded layout"
        );
        output
    }

    /// 无论载荷多大都强制 sidecar 布局：阈值 0 使预判与实际一致地走旁文件路径。
    /// Force the sidecar layout regardless of payload size: a threshold of 0
    /// sends both the prediction and the payload through the sidecar path.
    fn force_sidecar_sfx(temp: &Path, content: &[u8], password: Option<&str>) -> PathBuf {
        let source = temp.join("src");
        fs::write(&source, content).expect("write source");
        build_sfx(temp, &source, password, 0, "out.sfx.exe")
    }

    #[test]
    fn sfx_manifest_roundtrip_and_extracts_plain_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output = build_small_sfx(temp.path(), "plain.txt", None);

        let info = load_embedded_archive_info_from_path(&output)
            .expect("load info")
            .expect("embedded info");
        assert_eq!(info.default_extract_name, "plain.txt");
        assert!(!info.encrypted);

        let dest_root = temp.path().join("extract");
        let report = extract_embedded_archive_from_path(
            &output,
            EmbeddedDecompressRequest {
                output_path: Some(path_to_string(&dest_root)),
                password: None,
            },
            None,
            None,
        )
        .expect("extract sfx");
        assert!(report.output_path.ends_with("plain.txt"));
        assert_eq!(
            fs::read(dest_root.join("plain.txt")).expect("read extracted"),
            b"hello from sfx"
        );
    }

    #[test]
    fn sfx_extract_rejects_wrong_password() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("secret.txt");
        fs::write(&source, b"encrypted sfx").expect("write source");

        let output = build_sfx(
            temp.path(),
            &source,
            Some("pw123"),
            u64::MAX,
            "secret.sfx.exe",
        );

        let dest_root = temp.path().join("extract");
        let err = extract_embedded_archive_from_path(
            &output,
            EmbeddedDecompressRequest {
                output_path: Some(path_to_string(&dest_root)),
                password: Some("wrong".to_string()),
            },
            None,
            None,
        )
        .expect_err("wrong password should fail");
        assert!(err
            .chain()
            .any(|cause| cause.to_string().contains("解密失败")));
    }

    #[test]
    fn embedded_layout_reads_manifest_from_exe_end() {
        // 小载荷 -> 嵌入式布局 -> manifest 位于 EXE 末尾，无 sidecar。
        // Small payload -> embedded layout -> manifest lives at exe end, no sidecar.
        let temp = tempfile::tempdir().expect("tempdir");
        let output = build_small_sfx(temp.path(), "plain.txt", None);
        let manifest = read_embedded_manifest(&output)
            .expect("read")
            .expect("manifest present");
        assert!(!manifest.payload_in_sidecar);
        // payload_offset 即宿主 EXE 的大小；载荷紧随其后位于 EXE 内部。
        // payload_offset is the host exe size; payload follows it inside the exe.
        let host_len = fs::metadata(temp.path().join("template.exe"))
            .unwrap()
            .len();
        assert_eq!(manifest.payload_offset, host_len);
    }

    #[test]
    fn sidecar_layout_extracts_and_reports_sidecar() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output = force_sidecar_sfx(temp.path(), b"sidecar payload data", None);
        assert!(sidecar_path(&output).exists());

        let manifest = read_embedded_manifest(&output)
            .expect("read")
            .expect("manifest present");
        assert!(manifest.payload_in_sidecar);
        assert_eq!(manifest.payload_offset, 0);

        let dest = temp.path().join("out");
        extract_embedded_archive_from_path(
            &output,
            EmbeddedDecompressRequest {
                output_path: Some(path_to_string(&dest)),
                password: None,
            },
            None,
            None,
        )
        .expect("sidecar extract");
        assert_eq!(
            fs::read(dest.join("src")).expect("read"),
            b"sidecar payload data"
        );
    }

    #[test]
    fn sidecar_path_derivation() {
        assert_eq!(
            sidecar_path(Path::new("C:/foo/bar.sfx.exe")),
            PathBuf::from("C:/foo/bar.sfx.exe.payload")
        );
        assert_eq!(sidecar_path(Path::new("bar")), PathBuf::from("bar.payload"));
        assert_eq!(
            sidecar_path(Path::new("D:/数据/归档.exe")),
            PathBuf::from("D:/数据/归档.exe.payload")
        );
    }

    #[test]
    fn read_embedded_manifest_missing_sidecar_returns_none() {
        let temp = tempfile::tempdir().expect("tempdir");
        let exe = temp.path().join("plain.exe");
        fs::write(&exe, b"MZfake-host-without-trailer").expect("write exe");
        // 无侧车且 EXE 无尾标：普通 ZARC 模式 / No sidecar or EXE trailer: normal ZARC mode.
        assert!(read_embedded_manifest(&exe).expect("ok").is_none());
    }

    #[test]
    fn read_embedded_manifest_corrupt_sidecar_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let exe = temp.path().join("plain.exe");
        fs::write(&exe, b"MZfake-host").expect("write exe");
        // 侧车存在但容不下尾标 / Sidecar exists but is too short for a trailer.
        fs::write(sidecar_path(&exe), b"short").expect("write corrupt sidecar");
        let err = read_embedded_manifest(&exe).expect_err("corrupt sidecar should error");
        assert!(err.to_string().contains("已损坏"), "got: {err}");
    }

    #[test]
    fn extract_missing_sidecar_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output = force_sidecar_sfx(temp.path(), b"sidecar payload data", None);
        // 删除侧车后 SFX 不再可读 / Removing the sidecar makes the SFX unreadable.
        fs::remove_file(sidecar_path(&output)).expect("remove sidecar");
        let info = load_embedded_archive_info_from_path(&output)
            .unwrap()
            .unwrap();
        assert!(
            !info.payload_ready,
            "host marker must survive loss of sidecar"
        );

        let dest = temp.path().join("out");
        let err = extract_embedded_archive_from_path(
            &output,
            EmbeddedDecompressRequest {
                output_path: Some(path_to_string(&dest)),
                password: None,
            },
            None,
            None,
        )
        .expect_err("missing sidecar should error");
        assert!(
            err.to_string().contains("嵌入归档") || err.to_string().contains("数据文件"),
            "got: {err}"
        );
    }

    #[test]
    fn sfx_pair_rename_still_extracts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output = force_sidecar_sfx(temp.path(), b"renamed pair", None);

        // 成对重命名两个文件 / Rename both files as a pair.
        let new_exe = temp.path().join("renamed.exe");
        fs::rename(&output, &new_exe).expect("rename exe");
        fs::rename(sidecar_path(&output), sidecar_path(&new_exe)).expect("rename sidecar");

        let dest = temp.path().join("out");
        extract_embedded_archive_from_path(
            &new_exe,
            EmbeddedDecompressRequest {
                output_path: Some(path_to_string(&dest)),
                password: None,
            },
            None,
            None,
        )
        .expect("renamed pair should still extract");
        assert_eq!(fs::read(dest.join("src")).expect("read"), b"renamed pair");
    }

    #[test]
    fn manifest_allocation_is_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("bad.payload");
        let mut bytes = vec![0u8; 65537];
        bytes.extend_from_slice(SFX_MAGIC);
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&65537u64.to_le_bytes());
        fs::write(&path, bytes).unwrap();
        assert!(read_manifest_from_file(&path, true)
            .unwrap_err()
            .to_string()
            .contains("64 KiB"));
    }

    #[test]
    fn sanitize_extract_name_blocks_traversal() {
        assert_eq!(sanitize_extract_name("plain").unwrap(), "plain");
        assert_eq!(
            sanitize_extract_name("../../../etc/passwd").unwrap(),
            "passwd"
        );
        assert_eq!(
            sanitize_extract_name("C:\\Windows\\System32\\evil.dll").unwrap(),
            "evil.dll"
        );
        // Linux 也拒绝反斜杠，避免 Windows 构建的 SFX 跨平台越界。
        // Reject backslashes on Linux too, preventing traversal by Windows-built SFX files.
        assert_eq!(sanitize_extract_name("a\\b\\c").unwrap(), "c");
        assert_eq!(sanitize_extract_name("dir/").unwrap(), "dir");

        assert!(sanitize_extract_name("").is_err());
        assert!(sanitize_extract_name("   ").is_err());
        assert!(sanitize_extract_name("..").is_err());
        assert!(sanitize_extract_name("/").is_err());
        assert!(sanitize_extract_name("../..").is_err());
    }

    #[test]
    fn sfx_with_traversing_name_stays_inside_output_dir() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("payload.txt");
        fs::write(&source, b"contained").expect("write source");

        let archive = temp.path().join("payload.zst");
        compress_to_archive(&source, &archive, None);

        // 手工构建 manifest 指向用户目录之外的 SFX。
        // Build an SFX whose manifest targets outside the selected directory.
        let template = temp.path().join("template.exe");
        fs::write(&template, b"MZfake-host").expect("write template");
        let output = temp.path().join("evil.sfx.exe");
        let payload_length = fs::metadata(&archive).unwrap().len();
        let manifest = SfxManifest {
            payload_offset: 0,
            payload_length,
            encrypted: false,
            archive_kind: ArchiveKind::Zst,
            default_extract_name: "../escaped.txt".to_string(),
            source_name: "payload.txt".to_string(),
            created_by_version: env!("CARGO_PKG_VERSION").to_string(),
            payload_in_sidecar: true,
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let parent = output.parent().unwrap();
        write_sidecar_host(&template, &output, &manifest_bytes, None).expect("write host marker");
        write_sfx_container(
            &sidecar_path(&output),
            None,
            &archive,
            payload_length,
            &manifest_bytes,
            payload_length,
            parent,
        )
        .expect("write sidecar");

        let dest_root = temp.path().join("dest");
        extract_embedded_archive_from_path(
            &output,
            EmbeddedDecompressRequest {
                output_path: Some(path_to_string(&dest_root)),
                password: None,
            },
            None,
            None,
        )
        .expect("extract");

        assert!(
            dest_root.join("escaped.txt").exists(),
            "must land inside the chosen directory"
        );
        assert!(
            !temp.path().join("escaped.txt").exists(),
            "path traversal escaped the output directory"
        );
    }

    #[test]
    fn failed_sfx_extraction_preserves_existing_output() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output = build_small_sfx(temp.path(), "plain.txt", Some("pw123"));

        let dest_root = temp.path().join("dest");
        fs::create_dir_all(&dest_root).expect("create dest");
        // 预先存在的同名无关文件，失败时不得被误删。
        // Pre-existing unrelated file with the target name; it must not be
        // deleted on failure.
        let victim = dest_root.join("plain");
        fs::write(&victim, b"unrelated user data").expect("write victim");

        let err = extract_embedded_archive_from_path(
            &output,
            EmbeddedDecompressRequest {
                output_path: Some(path_to_string(&dest_root)),
                password: Some("wrong".to_string()),
            },
            None,
            None,
        )
        .expect_err("wrong password should fail");
        assert!(!full_error_chain(&err).is_empty());

        assert_eq!(
            fs::read(&victim).expect("victim must survive"),
            b"unrelated user data"
        );
    }

    #[test]
    fn failed_sfx_extraction_leaves_no_partial_tree() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("tree");
        fs::create_dir_all(source.join("nested")).expect("create tree");
        for index in 0..40 {
            fs::write(
                source.join(format!("nested/file{index}.bin")),
                vec![b'x'; 32 * 1024],
            )
            .expect("write file");
        }

        let archive = temp.path().join("tree.tar.zst");
        compress_to_archive(&source, &archive, None);

        // 截断负载，使 tar 解压中途失败 / Truncate the payload so tar extraction fails midway.
        let raw = fs::read(&archive).expect("read archive");
        let truncated = temp.path().join("tree.trunc.tar.zst");
        fs::write(&truncated, &raw[..raw.len() / 2]).expect("write truncated");

        // 手工封装嵌入式容器：[host][truncated payload][manifest][trailer]
        // Hand-pack the embedded container: [host][truncated payload][manifest][trailer].
        let template = temp.path().join("template.exe");
        fs::write(&template, b"MZfake-host").expect("write template");
        let output = temp.path().join("tree.sfx.exe");
        let host_len = fs::metadata(&template).unwrap().len();
        let payload_length = fs::metadata(&truncated).unwrap().len();
        let manifest = SfxManifest {
            payload_offset: host_len,
            payload_length,
            encrypted: false,
            archive_kind: ArchiveKind::TarZst,
            default_extract_name: "tree".to_string(),
            source_name: "tree".to_string(),
            created_by_version: env!("CARGO_PKG_VERSION").to_string(),
            payload_in_sidecar: false,
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let parent = output.parent().unwrap();
        write_sfx_container(
            &output,
            Some(&template),
            &truncated,
            payload_length,
            &manifest_bytes,
            host_len + payload_length,
            parent,
        )
        .expect("write container");

        let dest_root = temp.path().join("dest");
        extract_embedded_archive_from_path(
            &output,
            EmbeddedDecompressRequest {
                output_path: Some(path_to_string(&dest_root)),
                password: None,
            },
            None,
            None,
        )
        .expect_err("truncated payload should fail");

        let leftovers: Vec<_> = fs::read_dir(&dest_root)
            .expect("read dest")
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            leftovers.is_empty(),
            "partial extraction left files behind: {leftovers:?}"
        );
    }
}

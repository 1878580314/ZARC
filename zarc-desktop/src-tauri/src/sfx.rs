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
    let level = request.level.unwrap_or(8).clamp(1, 22);
    let include_root_dir = request.include_root_dir.unwrap_or(true);
    let password = normalize_password(request.password);
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

    let temp_dir = tempfile::tempdir().context("无法创建临时目录")?;
    let temp_archive = temp_dir.path().join(default_compress_file_name(
        &source,
        password.is_some(),
        OutputKind::Archive,
    )?);

    let started = Instant::now();
    let operation_result = if source.is_dir() {
        compress_directory(
            &source,
            &temp_archive,
            level,
            include_root_dir,
            password.as_deref(),
            &reporter,
            state.as_ref(),
            None,
        )
    } else {
        compress_file(
            &source,
            &temp_archive,
            level,
            password.as_deref(),
            &reporter,
            state.as_ref(),
            None,
        )
    };

    if let Err(err) = operation_result {
        cleanup_sfx_output(&output);
        reporter.fail(full_error_chain(&err));
        log_to_file(enable_logging, &format!("生成自解压包失败: {err:#}"));
        return Err(err);
    }

    let sidecar = match build_sfx_executable(&host_exe, &temp_archive, &output, &source) {
        Ok(sidecar) => sidecar,
        Err(err) => {
            cleanup_sfx_output(&output);
            reporter.fail(full_error_chain(&err));
            log_to_file(enable_logging, &format!("封装自解压 EXE 失败: {err:#}"));
            return Err(err);
        }
    };

    reporter.finish();

    let duration = started.elapsed().as_secs_f64();
    // For the sidecar layout, size/hash the payload file; for embedded, the exe
    // itself carries the payload so size/hash the exe.
    let (result_path, report_sidecar) = match &sidecar {
        Some(sc) => (sc.clone(), Some(path_to_string(sc))),
        None => (output.clone(), None),
    };
    let output_bytes = fs::metadata(&result_path)
        .with_context(|| format!("无法读取结果文件信息: {}", result_path.display()))?
        .len();
    let hash = calculate_file_hash(&result_path).ok();

    log_to_file(
        enable_logging,
        &format!(
            "生成自解压包完成. 原始大小: {}, 输出大小: {}, 耗时: {:.2}s",
            source_bytes, output_bytes, duration
        ),
    );

    if delete_source_after {
        log_to_file(enable_logging, &format!("正在删除源: {}", source.display()));
        if let Err(err) = delete_source_path(&source) {
            reporter.fail(full_error_chain(&err));
            log_to_file(enable_logging, &format!("删除源失败: {err:#}"));
            return Err(err);
        }
    }

    Ok(OperationReport {
        operation: "compress".to_string(),
        source_path: path_to_string(&source),
        output_path: path_to_string(&output),
        source_bytes,
        output_bytes,
        duration_ms: duration * 1000.0,
        throughput_mi_bs: throughput(source_bytes, duration),
        compression_ratio: Some(ratio(output_bytes, source_bytes)),
        blake3_hash: hash,
        sidecar_path: report_sidecar,
    })
}

fn build_sfx_executable(
    host_exe: &Path,
    archive_path: &Path,
    output_exe: &Path,
    source: &Path,
) -> Result<Option<PathBuf>> {
    let archive_meta = detect_archive_meta(archive_path)?;
    let payload_length = fs::metadata(archive_path)
        .with_context(|| format!("无法读取归档信息: {}", archive_path.display()))?
        .len();
    let default_extract_name = default_decompress_name(archive_path, archive_meta)?;
    let source_name = source
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());
    let created_by_version = env!("CARGO_PKG_VERSION").to_string();
    let parent = output_exe.parent().unwrap_or_else(|| Path::new("."));

    if payload_length >= SFX_SIDECAR_THRESHOLD {
        // Sidecar layout: exe stays a plain host copy, payload lives in .payload.
        let manifest = SfxManifest {
            payload_offset: 0,
            payload_length,
            encrypted: archive_meta.encrypted,
            archive_kind: archive_meta.kind,
            default_extract_name,
            source_name,
            created_by_version,
            payload_in_sidecar: true,
        };
        let manifest_bytes = serde_json::to_vec(&manifest).context("无法序列化 SFX manifest")?;
        copy_host_exe(host_exe, output_exe, parent)?;
        let sidecar = sidecar_path(output_exe);
        if let Err(err) = write_sidecar(
            &sidecar,
            archive_path,
            payload_length,
            &manifest_bytes,
            payload_length,
            parent,
        ) {
            let _ = fs::remove_file(output_exe);
            return Err(err);
        }
        Ok(Some(sidecar))
    } else {
        // Embedded layout: single file [host][payload][manifest][trailer].
        let host_len = fs::metadata(host_exe)
            .with_context(|| format!("无法读取宿主程序信息: {}", host_exe.display()))?
            .len();
        let payload_offset = host_len;
        let manifest = SfxManifest {
            payload_offset,
            payload_length,
            encrypted: archive_meta.encrypted,
            archive_kind: archive_meta.kind,
            default_extract_name,
            source_name,
            created_by_version,
            payload_in_sidecar: false,
        };
        let manifest_bytes = serde_json::to_vec(&manifest).context("无法序列化 SFX manifest")?;
        let manifest_offset = payload_offset
            .checked_add(payload_length)
            .with_context(|| "SFX 文件偏移超出范围")?;
        write_embedded_sfx(
            host_exe,
            archive_path,
            output_exe,
            payload_length,
            &manifest_bytes,
            manifest_offset,
            parent,
        )?;
        Ok(None)
    }
}

/// Atomically copy the host exe verbatim to `output_exe`. The host is a
/// legitimate PE; copying it unchanged keeps it loadable regardless of payload
/// size (the ~2GB single-image cap doesn't apply when payload is in a sidecar).
fn copy_host_exe(host_exe: &Path, output_exe: &Path, parent: &Path) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("无法在输出目录创建临时文件: {}", parent.display()))?;
    {
        let mut writer = BufWriter::with_capacity(IO_BUFFER_SIZE, temp.as_file_mut());
        let mut host_file = File::open(host_exe)
            .with_context(|| format!("无法打开宿主程序: {}", host_exe.display()))?;
        io::copy(&mut host_file, &mut writer).with_context(|| {
            format!(
                "复制宿主程序失败: {} -> {}",
                host_exe.display(),
                output_exe.display()
            )
        })?;
        writer.flush().context("刷新宿主副本失败")?;
    }
    temp.persist(output_exe)
        .map_err(|err| err.error)
        .with_context(|| format!("保存 SFX EXE 失败: {}", output_exe.display()))?;
    Ok(())
}

fn write_embedded_sfx(
    host_exe: &Path,
    archive_path: &Path,
    output_exe: &Path,
    payload_length: u64,
    manifest_bytes: &[u8],
    manifest_offset: u64,
    parent: &Path,
) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("无法在输出目录创建临时文件: {}", parent.display()))?;
    {
        let mut writer = BufWriter::with_capacity(IO_BUFFER_SIZE, temp.as_file_mut());
        let mut host_file = File::open(host_exe)
            .with_context(|| format!("无法打开宿主程序: {}", host_exe.display()))?;
        io::copy(&mut host_file, &mut writer)
            .with_context(|| format!("复制宿主程序失败: {}", host_exe.display()))?;
        copy_file_prefix(archive_path, payload_length, &mut writer)?;
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
    temp.persist(output_exe)
        .map_err(|err| err.error)
        .with_context(|| format!("保存 SFX 输出失败: {}", output_exe.display()))?;
    Ok(())
}

fn write_sidecar(
    sidecar: &Path,
    archive_path: &Path,
    payload_length: u64,
    manifest_bytes: &[u8],
    manifest_offset: u64,
    parent: &Path,
) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("无法创建侧车临时文件: {}", parent.display()))?;
    {
        let mut writer = BufWriter::with_capacity(IO_BUFFER_SIZE, temp.as_file_mut());
        copy_file_prefix(archive_path, payload_length, &mut writer)?;
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
        writer.flush().context("刷新 SFX 侧车失败")?;
    }
    temp.persist(sidecar)
        .map_err(|err| err.error)
        .with_context(|| format!("保存 SFX 侧车失败: {}", sidecar.display()))?;
    Ok(())
}

/// Path of the payload sidecar that accompanies an SFX exe.
fn sidecar_path(exe: &Path) -> PathBuf {
    let mut os_string = exe.as_os_str().to_owned();
    os_string.push(SIDECAR_SUFFIX);
    PathBuf::from(os_string)
}

/// Remove both the SFX exe and its sidecar (best-effort, for failure cleanup).
fn cleanup_sfx_output(exe: &Path) {
    let _ = fs::remove_file(exe);
    let _ = fs::remove_file(sidecar_path(exe));
}

fn full_error_chain(err: &anyhow::Error) -> String {
    // anyhow's alternate Display joins the context chain with ": ", so the OS-level
    // cause (e.g. "No space left on device (os error 28)") is preserved instead of
    // being dropped by the top-level context message.
    format!("{err:#}")
}

fn copy_file_prefix(path: &Path, length: u64, output: &mut impl Write) -> Result<()> {
    let input = File::open(path).with_context(|| format!("无法打开文件: {}", path.display()))?;
    let mut reader = BufReader::with_capacity(IO_BUFFER_SIZE, input.take(length));
    io::copy(&mut reader, output)
        .with_context(|| format!("复制文件内容失败: {}", path.display()))?;
    Ok(())
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
    let output = output_root.join(&manifest.default_extract_name);

    let payload_path = if manifest.payload_in_sidecar {
        let sidecar = sidecar_path(host_path);
        if !sidecar.exists() {
            bail!(
                "找不到数据文件 {}，请确保它与 EXE 放在一起",
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
    let buf_reader = BufReader::with_capacity(IO_BUFFER_SIZE, section_reader);
    let progress_reader = ProgressReader::new(buf_reader, reporter.clone());

    let output_result = match (manifest.encrypted, manifest.archive_kind) {
        (true, ArchiveKind::TarZst) => {
            fs::create_dir_all(&output)
                .with_context(|| format!("无法创建解压目录: {}", output.display()))?;
            let decrypt_reader =
                EncryptedReader::new(progress_reader, password.as_deref().unwrap_or_default())?;
            decompress_tar_from_reader(decrypt_reader, &output, state.as_ref())?;
            Ok(count_source_bytes(&output)?)
        }
        (true, ArchiveKind::Zst) => {
            let decrypt_reader =
                EncryptedReader::new(progress_reader, password.as_deref().unwrap_or_default())?;
            decompress_file_from_reader(decrypt_reader, &output, state.as_ref())
        }
        (false, ArchiveKind::TarZst) => {
            fs::create_dir_all(&output)
                .with_context(|| format!("无法创建解压目录: {}", output.display()))?;
            decompress_tar_from_reader(progress_reader, &output, state.as_ref())?;
            Ok(count_source_bytes(&output)?)
        }
        (false, ArchiveKind::Zst) => {
            decompress_file_from_reader(progress_reader, &output, state.as_ref())
        }
    };

    let output_bytes = match output_result {
        Ok(bytes) => bytes,
        Err(err) => {
            if output.is_dir() {
                let _ = fs::remove_dir_all(&output);
            } else {
                let _ = fs::remove_file(&output);
            }
            reporter.fail(full_error_chain(&err));
            return Err(err);
        }
    };

    reporter.finish();
    let duration = started.elapsed().as_secs_f64();
    let hash = calculate_file_hash(&payload_path).ok();

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

fn read_embedded_manifest(exe_path: &Path) -> Result<Option<SfxManifest>> {
    let sidecar = sidecar_path(exe_path);
    if sidecar.exists() {
        // Sidecar present: any corruption is a hard error (don't silently fall back
        // to plain ZARC mode — the user clearly intended an SFX here).
        return read_manifest_from_file(&sidecar, true);
    }
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
    let mut manifest_bytes = vec![0_u8; manifest_len];
    file.read_exact(&mut manifest_bytes)
        .context("无法读取 SFX manifest")?;
    let manifest: SfxManifest =
        serde_json::from_slice(&manifest_bytes).context("无法解析 SFX manifest")?;
    let payload_end = manifest
        .payload_offset
        .checked_add(manifest.payload_length)
        .with_context(|| "SFX payload 长度非法")?;
    if payload_end > manifest_offset {
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

    /// Build an SFX from a small source via the production path. Returns the exe
    /// path; small payloads take the embedded (single-file) layout.
    fn build_small_sfx(temp: &Path, source_name: &str, archive_name: &str, password: Option<&str>) -> PathBuf {
        let source = temp.join(source_name);
        fs::write(&source, b"hello from sfx").expect("write source");

        let archive = temp.join(archive_name);
        let reporter =
            ProgressReporter::new(None, "compress", fs::metadata(&source).unwrap().len());
        compress_file(&source, &archive, 8, password, &reporter, None, None).expect("compress");

        let template = temp.join("template.exe");
        fs::write(&template, b"MZfake-host").expect("write template");
        let output = temp.join("plain.sfx.exe");
        let sidecar = build_sfx_executable(&template, &archive, &output, &source).expect("build sfx");
        assert!(sidecar.is_none(), "small payload should embed, not sidecar");
        assert!(!sidecar_path(&output).exists(), "no sidecar for embedded layout");
        output
    }

    /// Force the sidecar layout regardless of payload size, by calling the
    /// sidecar build helpers directly (bypassing the threshold check).
    fn force_sidecar_sfx(temp: &Path, content: &[u8], password: Option<&str>) -> PathBuf {
        let source = temp.join("src.txt");
        fs::write(&source, content).expect("write source");
        let archive = temp.join("src.zst");
        let reporter =
            ProgressReporter::new(None, "compress", fs::metadata(&source).unwrap().len());
        compress_file(&source, &archive, 8, password, &reporter, None, None).expect("compress");

        let template = temp.join("template.exe");
        fs::write(&template, b"MZfake-host").expect("write template");
        let output = temp.join("out.sfx.exe");

        let archive_meta = detect_archive_meta(&archive).unwrap();
        let payload_length = fs::metadata(&archive).unwrap().len();
        let manifest = SfxManifest {
            payload_offset: 0,
            payload_length,
            encrypted: archive_meta.encrypted,
            archive_kind: archive_meta.kind,
            default_extract_name: default_decompress_name(&archive, archive_meta).unwrap(),
            source_name: "src".to_string(),
            created_by_version: env!("CARGO_PKG_VERSION").to_string(),
            payload_in_sidecar: true,
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let parent = output.parent().unwrap();
        copy_host_exe(&template, &output, parent).expect("copy host");
        let sidecar = sidecar_path(&output);
        write_sidecar(&sidecar, &archive, payload_length, &manifest_bytes, payload_length, parent)
            .expect("write sidecar");
        output
    }

    #[test]
    fn sfx_manifest_roundtrip_and_extracts_plain_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output = build_small_sfx(temp.path(), "plain.txt", "plain.zst", None);

        let info = load_embedded_archive_info_from_path(&output)
            .expect("load info")
            .expect("embedded info");
        assert_eq!(info.default_extract_name, "plain");
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
        assert!(report.output_path.ends_with("plain"));
        assert_eq!(
            fs::read(dest_root.join("plain")).expect("read extracted"),
            b"hello from sfx"
        );
    }

    #[test]
    fn sfx_extract_rejects_wrong_password() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("secret.txt");
        fs::write(&source, b"encrypted sfx").expect("write source");

        let archive = temp.path().join("secret.zst.enc");
        let reporter =
            ProgressReporter::new(None, "compress", fs::metadata(&source).unwrap().len());
        compress_file(&source, &archive, 8, Some("pw123"), &reporter, None, None)
            .expect("compress");

        let template = temp.path().join("template.exe");
        fs::write(&template, b"MZfake-host").expect("write template");
        let output = temp.path().join("secret.sfx.exe");
        build_sfx_executable(&template, &archive, &output, &source).expect("build sfx");

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
        // Small payload -> embedded layout -> manifest lives at exe end, no sidecar.
        let temp = tempfile::tempdir().expect("tempdir");
        let output = build_small_sfx(temp.path(), "plain.txt", "plain.zst", None);
        let manifest = read_embedded_manifest(&output)
            .expect("read")
            .expect("manifest present");
        assert!(!manifest.payload_in_sidecar);
        // payload_offset is the host exe size; payload follows it inside the exe.
        let host_len = fs::metadata(temp.path().join("template.exe")).unwrap().len();
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
        assert_eq!(fs::read(dest.join("src")).expect("read"), b"sidecar payload data");
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
        // no sidecar, exe has no trailer -> normal ZARC mode
        assert!(read_embedded_manifest(&exe).expect("ok").is_none());
    }

    #[test]
    fn read_embedded_manifest_corrupt_sidecar_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let exe = temp.path().join("plain.exe");
        fs::write(&exe, b"MZfake-host").expect("write exe");
        // sidecar present but too short to hold a trailer
        fs::write(sidecar_path(&exe), b"short").expect("write corrupt sidecar");
        let err = read_embedded_manifest(&exe).expect_err("corrupt sidecar should error");
        assert!(err.to_string().contains("已损坏"), "got: {err}");
    }

    #[test]
    fn extract_missing_sidecar_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output = force_sidecar_sfx(temp.path(), b"sidecar payload data", None);
        // remove the sidecar -> SFX can no longer be read
        fs::remove_file(sidecar_path(&output)).expect("remove sidecar");

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

        // rename both files as a pair
        let new_exe = temp.path().join("renamed.exe");
        fs::rename(&output, &new_exe).expect("rename exe");
        fs::rename(sidecar_path(&output), sidecar_path(&new_exe))
            .expect("rename sidecar");

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
}

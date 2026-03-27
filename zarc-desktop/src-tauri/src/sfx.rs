use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::*;

const SFX_MAGIC: &[u8; 8] = b"ZARCSFX1";
const SFX_TRAILER_LEN: u64 = 24;

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
        reporter.fail(err.to_string());
        return Err(err);
    }

    let source = PathBuf::from(request.source_path.trim());
    let level = request.level.unwrap_or(8).clamp(1, 22);
    let include_root_dir = request.include_root_dir.unwrap_or(true);
    let password = normalize_password(request.password);
    let host_exe = std::env::current_exe().context("无法定位当前程序")?;
    let host_template_len = host_template_length(&host_exe)?;
    if output == host_exe {
        let err = anyhow!("输出路径不能覆盖当前运行中的程序");
        reporter.fail(err.to_string());
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
        let _ = fs::remove_file(&output);
        reporter.fail(err.to_string());
        log_to_file(enable_logging, &format!("生成自解压包失败: {}", err));
        return Err(err);
    }

    if let Err(err) = build_sfx_executable(
        &host_exe,
        host_template_len,
        &temp_archive,
        &output,
        &source,
    ) {
        let _ = fs::remove_file(&output);
        reporter.fail(err.to_string());
        log_to_file(enable_logging, &format!("封装自解压 EXE 失败: {}", err));
        return Err(err);
    }

    reporter.finish();

    let duration = started.elapsed().as_secs_f64();
    let output_bytes = fs::metadata(&output)
        .with_context(|| format!("无法读取结果文件信息: {}", output.display()))?
        .len();
    let hash = calculate_file_hash(&output).ok();

    log_to_file(
        enable_logging,
        &format!(
            "生成自解压包完成. 原始大小: {}, 输出大小: {}, 耗时: {:.2}s",
            source_bytes, output_bytes, duration
        ),
    );

    if delete_source_after {
        log_to_file(enable_logging, &format!("正在删除源: {}", source.display()));
        if source.is_dir() {
            let _ = fs::remove_dir_all(&source);
        } else {
            let _ = fs::remove_file(&source);
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
    })
}

fn build_sfx_executable(
    host_exe: &Path,
    host_template_len: u64,
    archive_path: &Path,
    output: &Path,
    source: &Path,
) -> Result<()> {
    let archive_meta = detect_archive_meta(archive_path)?;
    let payload_length = fs::metadata(archive_path)
        .with_context(|| format!("无法读取归档信息: {}", archive_path.display()))?
        .len();

    let manifest = SfxManifest {
        payload_offset: host_template_len,
        payload_length,
        encrypted: archive_meta.encrypted,
        archive_kind: archive_meta.kind,
        default_extract_name: default_decompress_name(archive_path, archive_meta)?,
        source_name: source
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive".to_string()),
        created_by_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).context("无法序列化 SFX manifest")?;
    let manifest_offset = host_template_len
        .checked_add(payload_length)
        .with_context(|| "SFX 文件偏移超出范围")?;

    let mut writer = BufWriter::with_capacity(
        IO_BUFFER_SIZE,
        File::create(output).with_context(|| format!("无法创建输出文件: {}", output.display()))?,
    );
    copy_file_prefix(host_exe, host_template_len, &mut writer)?;
    copy_file_prefix(archive_path, payload_length, &mut writer)?;
    writer
        .write_all(&manifest_bytes)
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
    Ok(())
}

fn copy_file_prefix(path: &Path, length: u64, output: &mut impl Write) -> Result<()> {
    let input = File::open(path).with_context(|| format!("无法打开文件: {}", path.display()))?;
    let mut reader = BufReader::with_capacity(IO_BUFFER_SIZE, input.take(length));
    io::copy(&mut reader, output)
        .with_context(|| format!("复制文件内容失败: {}", path.display()))?;
    Ok(())
}

fn host_template_length(path: &Path) -> Result<u64> {
    if let Some(manifest) = read_embedded_manifest(path)? {
        return Ok(manifest.payload_offset);
    }
    Ok(fs::metadata(path)
        .with_context(|| format!("无法读取宿主程序信息: {}", path.display()))?
        .len())
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

    let source_bytes = fs::metadata(host_path)
        .with_context(|| format!("无法读取自解压文件信息: {}", host_path.display()))?
        .len();

    let reporter = ProgressReporter::new(app, "decompress", manifest.payload_length);
    reporter.begin();
    let started = Instant::now();

    let mut file = File::open(host_path)
        .with_context(|| format!("无法打开自解压文件: {}", host_path.display()))?;
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
            reporter.fail(err.to_string());
            return Err(err);
        }
    };

    reporter.finish();
    let duration = started.elapsed().as_secs_f64();
    let hash = calculate_file_hash(host_path).ok();

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
    })
}

fn read_embedded_manifest(path: &Path) -> Result<Option<SfxManifest>> {
    let metadata =
        fs::metadata(path).with_context(|| format!("无法读取文件信息: {}", path.display()))?;
    if metadata.len() < SFX_TRAILER_LEN {
        return Ok(None);
    }

    let mut file = File::open(path).with_context(|| format!("无法打开文件: {}", path.display()))?;
    file.seek(SeekFrom::End(-(SFX_TRAILER_LEN as i64)))
        .with_context(|| "无法定位 SFX trailer")?;

    let mut magic = [0_u8; SFX_MAGIC.len()];
    file.read_exact(&mut magic)
        .context("无法读取 SFX trailer magic")?;
    if &magic != SFX_MAGIC {
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
    let mut manifest_bytes = vec![0_u8; manifest_length as usize];
    file.read_exact(&mut manifest_bytes)
        .context("无法读取 SFX manifest")?;
    let manifest: SfxManifest =
        serde_json::from_slice(&manifest_bytes).context("无法解析 SFX manifest")?;
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

    #[test]
    fn sfx_manifest_roundtrip_and_extracts_plain_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("plain.txt");
        fs::write(&source, b"hello from sfx").expect("write source");

        let archive = temp.path().join("plain.zst");
        let reporter =
            ProgressReporter::new(None, "compress", fs::metadata(&source).unwrap().len());
        compress_file(&source, &archive, 8, None, &reporter, None, None).expect("compress");

        let template = temp.path().join("template.exe");
        fs::write(&template, b"MZfake-host").expect("write template");
        let output = temp.path().join("plain.sfx.exe");
        build_sfx_executable(
            &template,
            fs::metadata(&template).unwrap().len(),
            &archive,
            &output,
            &source,
        )
        .expect("build sfx");

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
        build_sfx_executable(
            &template,
            fs::metadata(&template).unwrap().len(),
            &archive,
            &output,
            &source,
        )
        .expect("build sfx");

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
        assert!(err.to_string().contains("解密失败"));
    }
}

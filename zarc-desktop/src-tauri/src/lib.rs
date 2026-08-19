use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{AeadInPlace, KeyInit, OsRng};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;
use zeroize::Zeroize;

mod sfx;

const IO_BUFFER_SIZE: usize = 8 * 1024 * 1024;
const MIB: f64 = 1024.0 * 1024.0;
const PROGRESS_EVENT: &str = "zarc://progress";
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(120);

/// Legacy header: KDF parameters were implicit (see `LEGACY_KDF`). Still read for
/// backward compatibility, never written.
const ENC_MAGIC_V1: &[u8; 8] = b"ZENC0001";
/// Current header: KDF parameters are stored explicitly so future tuning cannot
/// silently turn old archives into "wrong password" errors.
const ENC_MAGIC_V2: &[u8; 8] = b"ZENC0002";
const ENC_SALT_LEN: usize = 16;
const ENC_NONCE_PREFIX_LEN: usize = 16;
const ENC_KEY_LEN: usize = 32;
const ENC_CHUNK_SIZE: usize = 256 * 1024;
const ENC_TAG_LEN: usize = 16;
/// Hard upper bound for a chunk length read off disk. A hostile/corrupt archive
/// must not be able to make us allocate an arbitrary buffer.
const ENC_MAX_CHUNK_LEN: usize = ENC_CHUNK_SIZE + ENC_TAG_LEN;

/// Argon2id parameters used by `ZENC0001` archives, which did not record them.
const LEGACY_KDF: KdfParams = KdfParams {
    m_cost_kib: 32 * 1024,
    t_cost: 2,
    parallelism: 1,
};
/// Argon2id parameters written into new archives.
const CURRENT_KDF: KdfParams = LEGACY_KDF;
/// Sanity bounds applied to parameters read from an archive header, so a forged
/// header cannot trigger a multi-gigabyte Argon2 allocation.
const KDF_MAX_M_COST_KIB: u32 = 1024 * 1024;
const KDF_MAX_T_COST: u32 = 64;
const KDF_MAX_PARALLELISM: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KdfParams {
    m_cost_kib: u32,
    t_cost: u32,
    parallelism: u32,
}

impl KdfParams {
    fn to_bytes(self) -> [u8; 12] {
        let mut out = [0_u8; 12];
        out[..4].copy_from_slice(&self.m_cost_kib.to_be_bytes());
        out[4..8].copy_from_slice(&self.t_cost.to_be_bytes());
        out[8..].copy_from_slice(&self.parallelism.to_be_bytes());
        out
    }

    fn from_bytes(raw: [u8; 12]) -> Result<Self> {
        let params = Self {
            m_cost_kib: u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]),
            t_cost: u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]),
            parallelism: u32::from_be_bytes([raw[8], raw[9], raw[10], raw[11]]),
        };

        if params.m_cost_kib == 0
            || params.t_cost == 0
            || params.parallelism == 0
            || params.m_cost_kib > KDF_MAX_M_COST_KIB
            || params.t_cost > KDF_MAX_T_COST
            || params.parallelism > KDF_MAX_PARALLELISM
        {
            bail!(
                "归档头中的密钥派生参数超出支持范围 (m={} KiB, t={}, p={})",
                params.m_cost_kib,
                params.t_cost,
                params.parallelism
            );
        }

        Ok(params)
    }
}

/// A derived key that scrubs itself on drop, so it does not linger in freed heap
/// or stack memory after the cipher has been built.
struct SecretKey([u8; ENC_KEY_LEN]);

impl SecretKey {
    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(Key::from_slice(&self.0))
    }
}

/// The normalized password, scrubbed when it goes out of scope.
///
/// `Deref<Target = str>` is deliberate: it makes `Option<SecretString>::as_deref()`
/// work, so every existing `password.as_deref()` call site keeps compiling while
/// gaining the wipe-on-drop behaviour.
struct SecretString(String);

impl std::ops::Deref for SecretString {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString(***)")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ArchiveEntry {
    path: String,
    size: u64,
    is_dir: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveContentReport {
    entries: Vec<ArchiveEntry>,
    total_files: usize,
    uncompressed_size: u64,
    hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompressRequest {
    source_path: String,
    output_path: Option<String>,
    output_kind: Option<OutputKind>,
    level: Option<i32>,
    include_root_dir: Option<bool>,
    password: Option<String>,
    split_size_mib: Option<u64>,
    enable_logging: Option<bool>,
    delete_source_after: Option<bool>,
    /// zstd worker threads. `None` / out-of-range falls back to every core.
    threads: Option<u32>,
}

/// Clamp a requested worker count into `1..=cores`, defaulting to all cores.
fn resolve_threads(requested: Option<u32>) -> u32 {
    let cores = num_cpus::get().max(1) as u32;
    requested.unwrap_or(cores).clamp(1, cores)
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum OutputKind {
    Archive,
    SfxExe,
}

impl OutputKind {
    fn archive_or_default(raw: Option<Self>) -> Self {
        raw.unwrap_or(Self::Archive)
    }
}

struct MultiVolumeWriter {
    base_path: PathBuf,
    current_file: Option<BufWriter<File>>,
    current_index: usize,
    bytes_written_in_volume: u64,
    volume_limit: u64,
    total_written: u64,
}

impl MultiVolumeWriter {
    fn new(base_path: PathBuf, volume_limit_mib: u64) -> Self {
        Self {
            base_path,
            current_file: None,
            current_index: 1,
            bytes_written_in_volume: 0,
            // `saturating_mul`: a nonsense MiB value must degrade into "one huge
            // volume", never wrap around into a tiny limit that would shred the
            // archive into millions of files.
            volume_limit: volume_limit_mib.saturating_mul(1024 * 1024),
            total_written: 0,
        }
    }

    fn ensure_file(&mut self) -> io::Result<&mut BufWriter<File>> {
        if self.current_file.is_none() {
            let path = self.volume_path(self.current_index);
            let file = File::create(path)?;
            self.current_file = Some(BufWriter::with_capacity(IO_BUFFER_SIZE, file));
        }
        Ok(self.current_file.as_mut().unwrap())
    }

    /// Close the current volume (durably) and move on to the next index.
    fn rotate_volume(&mut self) -> io::Result<()> {
        if let Some(mut f) = self.current_file.take() {
            f.flush()?;
            f.get_ref().sync_all()?;
        }
        self.current_index += 1;
        self.bytes_written_in_volume = 0;
        Ok(())
    }

    fn sync_all(&mut self) -> io::Result<()> {
        if let Some(ref mut f) = self.current_file {
            f.flush()?;
            f.get_ref().sync_all()?;
        }
        Ok(())
    }

    fn volume_path(&self, index: usize) -> PathBuf {
        let ext = format!("{:03}", index);
        let mut path = self.base_path.clone();
        let base_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        path.set_file_name(format!("{base_name}.{ext}"));
        path
    }
}

impl Write for MultiVolumeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.volume_limit == 0 {
            let writer = self.ensure_file()?;
            let n = writer.write(buf)?;
            self.total_written += n as u64;
            return Ok(n);
        }

        let mut written = 0;
        while written < buf.len() {
            if self.bytes_written_in_volume >= self.volume_limit {
                self.rotate_volume()?;
            }

            // Recomputed *after* the rotation. Reading it before meant the first
            // write into a fresh volume saw `remaining == 0` and fell back to
            // `.max(1)`, i.e. one byte per syscall for the whole archive.
            let remaining_in_vol = self.volume_limit - self.bytes_written_in_volume;
            let take = ((buf.len() - written) as u64).min(remaining_in_vol) as usize;

            let writer = self.ensure_file()?;
            let n = writer.write(&buf[written..written + take])?;
            if n == 0 {
                break;
            }

            written += n;
            self.bytes_written_in_volume += n as u64;
            self.total_written += n as u64;
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut f) = self.current_file {
            f.flush()?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecompressRequest {
    archive_path: String,
    output_path: Option<String>,
    password: Option<String>,
}

/// Upper bound on the files counted by [`inspect_path`]; past this the UI shows
/// a "≥" estimate instead of freezing on a million-entry tree.
const PATH_INSPECT_ENTRY_CAP: u64 = 200_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PathInfo {
    path: String,
    exists: bool,
    is_dir: bool,
    size_bytes: u64,
    file_count: u64,
    /// `true` when the walk hit [`PATH_INSPECT_ENTRY_CAP`] and the totals are lower bounds.
    truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbeddedDecompressRequest {
    output_path: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkRequest {
    source_path: String,
    min_level: Option<u8>,
    max_level: Option<u8>,
    iterations: Option<u32>,
    sample_size_mib: Option<u32>,
    threads: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationReport {
    operation: String,
    source_path: String,
    output_path: String,
    source_bytes: u64,
    output_bytes: u64,
    duration_ms: f64,
    throughput_mi_bs: f64,
    compression_ratio: Option<f64>,
    blake3_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sidecar_path: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EmbeddedArchiveInfo {
    host_path: String,
    payload_bytes: u64,
    default_extract_name: String,
    encrypted: bool,
    archive_kind: String,
}

#[tauri::command]
async fn list_archive_content(
    app: AppHandle,
    state: State<'_, AppState>,
    request: DecompressRequest,
) -> std::result::Result<ArchiveContentReport, String> {
    state.reset_abort();
    let state_inner = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        list_archive_content_sync(request, Some(app), Some(state_inner))
    })
    .await
    .map_err(|err| format!("任务线程异常: {err}"))?
    .map_err(|err| err.to_string())
}

fn list_archive_content_sync(
    request: DecompressRequest,
    app: Option<AppHandle>,
    state: Option<AppState>,
) -> Result<ArchiveContentReport> {
    let archive = PathBuf::from(request.archive_path.trim());
    if !archive.exists() {
        bail!("归档文件不存在: {}", archive.display());
    }

    let meta = detect_archive_meta(&archive)?;
    let password = normalize_password(request.password);
    if meta.encrypted && password.is_none() {
        bail!("该归档已加密，请提供解密密码以预览内容");
    }

    let archive_bytes = archive_input_bytes(&archive, meta)?;
    // Two passes over the archive: hash, then decode. Reporting `2 ×` up front
    // keeps the bar monotonic instead of snapping back to 50% halfway through.
    let reporter = ProgressReporter::new(app, "decompress", archive_bytes.saturating_mul(2));
    reporter.begin();

    let result = list_archive_content_inner(&archive, meta, password.as_deref(), &reporter, state);
    match result {
        Ok(report) => {
            reporter.finish();
            Ok(report)
        }
        Err(err) => {
            reporter.fail(err.to_string());
            Err(err)
        }
    }
}

fn list_archive_content_inner(
    archive: &Path,
    meta: ArchiveMeta,
    password: Option<&str>,
    reporter: &ProgressReporter,
    state: Option<AppState>,
) -> Result<ArchiveContentReport> {
    // Pass 1 — digest every volume, not just `.001`.
    let mut hasher = blake3::Hasher::new();
    let mut hash_buf = vec![0_u8; 1024 * 1024];
    for volume in archive_volume_paths(archive, meta)? {
        let mut file = File::open(&volume)
            .with_context(|| format!("无法打开归档文件: {}", volume.display()))?;
        loop {
            if let Some(s) = &state {
                if s.is_aborted() {
                    bail!("用户已终止任务");
                }
            }
            let read = file
                .read(&mut hash_buf)
                .with_context(|| format!("读取归档失败: {}", volume.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&hash_buf[..read]);
            reporter.advance(read as u64);
        }
    }
    let archive_hash = hasher.finalize().to_hex().to_string();

    // Pass 2 — decode. `ProgressReader` drives the bar; `AbortableReader` makes
    // the Stop button work, which it previously did not for preview at all.
    let reader: Box<dyn Read> = if meta.is_multi_volume {
        Box::new(MultiVolumeReader::new(archive.to_path_buf()))
    } else {
        Box::new(
            File::open(archive)
                .with_context(|| format!("无法打开归档文件: {}", archive.display()))?,
        )
    };
    let buf_reader = BufReader::with_capacity(IO_BUFFER_SIZE, reader);
    let progress_reader = ProgressReader::new(buf_reader, reporter.clone());
    let abortable = AbortableReader::new(progress_reader, state.as_ref());

    let mut entries = Vec::new();
    let mut total_size = 0_u64;

    if meta.encrypted {
        let decrypt_reader = EncryptedReader::new(abortable, password.unwrap_or_default())?;
        read_archive_listing(decrypt_reader, archive, meta, &mut entries, &mut total_size)?;
    } else {
        read_archive_listing(abortable, archive, meta, &mut entries, &mut total_size)?;
    }

    let total_files = entries.iter().filter(|entry| !entry.is_dir).count();
    Ok(ArchiveContentReport {
        entries,
        total_files,
        uncompressed_size: total_size,
        hash: archive_hash,
    })
}

fn read_archive_listing<R: Read>(
    reader: R,
    archive: &Path,
    meta: ArchiveMeta,
    entries: &mut Vec<ArchiveEntry>,
    total_size: &mut u64,
) -> Result<()> {
    match meta.kind {
        ArchiveKind::TarZst => {
            let decoder = zstd::Decoder::new(reader).context("创建 zstd 解码器失败")?;
            let mut tar = tar::Archive::new(decoder);
            for entry in tar.entries().context("读取归档目录失败")? {
                let entry = entry.context("读取归档条目失败")?;
                let is_dir = entry.header().entry_type().is_dir();
                let size = entry.size();
                entries.push(ArchiveEntry {
                    path: entry.path().context("归档条目路径无效")?.to_string_lossy().to_string(),
                    size,
                    is_dir,
                });
                *total_size = total_size.saturating_add(size);
            }
        }
        ArchiveKind::Zst => {
            // A single-file `.zst` carries no name or size in the frame, so the
            // only honest way to report them is to decode and count. Doing that
            // also means a wrong password now *fails* here — before, this arm
            // ignored the password entirely and happily reported `size: 0`.
            let mut decoder = zstd::Decoder::new(reader).context("创建 zstd 解码器失败")?;
            let mut buffer = vec![0_u8; 512 * 1024];
            let mut size = 0_u64;
            loop {
                let read = decoder.read(&mut buffer).context("解压读取失败")?;
                if read == 0 {
                    break;
                }
                size = size.saturating_add(read as u64);
            }

            entries.push(ArchiveEntry {
                path: default_decompress_name(&volume_base_path(archive), meta)
                    .unwrap_or_else(|_| "output".to_string()),
                size,
                is_dir: false,
            });
            *total_size = total_size.saturating_add(size);
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CompressionLevelReport {
    level: u8,
    mean_ms: f64,
    mean_throughput_mi_bs: f64,
    compressed_bytes: u64,
    ratio_percent: f64,
    score: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkReport {
    source_path: String,
    sample_bytes: u64,
    min_level: u8,
    max_level: u8,
    iterations: u32,
    threads: u32,
    recommended_level: u8,
    results: Vec<CompressionLevelReport>,
    note: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    operation: String,
    processed_bytes: u64,
    total_bytes: u64,
    percent: f64,
    throughput_mi_bs: f64,
    eta_seconds: Option<f64>,
    done: bool,
    error: Option<String>,
}

#[derive(Clone)]
struct AppState {
    abort_requested: Arc<AtomicBool>,
}

impl AppState {
    fn new() -> Self {
        Self {
            abort_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    fn request_abort(&self) {
        self.abort_requested.store(true, AtomicOrdering::SeqCst);
    }

    fn reset_abort(&self) {
        self.abort_requested.store(false, AtomicOrdering::SeqCst);
    }

    fn is_aborted(&self) -> bool {
        self.abort_requested.load(AtomicOrdering::SeqCst)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ArchiveKind {
    TarZst,
    Zst,
}

#[derive(Debug, Copy, Clone)]
struct ArchiveMeta {
    kind: ArchiveKind,
    encrypted: bool,
    is_multi_volume: bool,
}

struct ProgressState {
    started: Instant,
    processed: AtomicU64,
    last_emit: Mutex<Instant>,
}

#[derive(Clone)]
struct ProgressReporter {
    app: Option<AppHandle>,
    operation: &'static str,
    total: u64,
    state: Arc<ProgressState>,
}

impl ProgressReporter {
    fn new(app: Option<AppHandle>, operation: &'static str, total: u64) -> Self {
        let now = Instant::now();
        Self {
            app,
            operation,
            total,
            state: Arc::new(ProgressState {
                started: now,
                processed: AtomicU64::new(0),
                // Backdate so the first `advance` is not throttled. `checked_sub`
                // because subtracting from a fresh monotonic clock can underflow.
                last_emit: Mutex::new(now.checked_sub(PROGRESS_EMIT_INTERVAL).unwrap_or(now)),
            }),
        }
    }

    fn begin(&self) {
        self.emit(false, None, true);
    }

    fn advance(&self, delta: u64) {
        if delta > 0 {
            self.state
                .processed
                .fetch_add(delta, AtomicOrdering::Relaxed);
            self.emit(false, None, false);
        }
    }

    fn finish(&self) {
        self.state
            .processed
            .store(self.total, AtomicOrdering::Relaxed);
        self.emit(true, None, true);
    }

    fn fail(&self, message: String) {
        self.emit(true, Some(message), true);
    }

    fn emit(&self, done: bool, error: Option<String>, force: bool) {
        if self.app.is_none() {
            return;
        }

        {
            let mut last_emit = self
                .state
                .last_emit
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if !force && !done && last_emit.elapsed() < PROGRESS_EMIT_INTERVAL {
                return;
            }
            *last_emit = Instant::now();
        }

        let processed = self
            .state
            .processed
            .load(AtomicOrdering::Relaxed)
            .min(self.total);

        let elapsed = self.state.started.elapsed().as_secs_f64().max(f64::EPSILON);
        let throughput = throughput(processed, elapsed);
        let percent = if self.total == 0 {
            100.0
        } else {
            processed as f64 / self.total as f64 * 100.0
        };

        let eta_seconds = if done || throughput <= 0.0 || processed >= self.total {
            None
        } else {
            let remaining_mib = (self.total.saturating_sub(processed) as f64) / MIB;
            Some(remaining_mib / throughput)
        };

        let payload = ProgressPayload {
            operation: self.operation.to_string(),
            processed_bytes: processed,
            total_bytes: self.total,
            percent: percent.clamp(0.0, 100.0),
            throughput_mi_bs: throughput,
            eta_seconds,
            done,
            error,
        };

        if let Some(app) = &self.app {
            let _ = app.emit(PROGRESS_EVENT, payload);
        }
    }
}

struct ProgressReader<R> {
    inner: R,
    reporter: ProgressReporter,
}

impl<R> ProgressReader<R> {
    fn new(inner: R, reporter: ProgressReporter) -> Self {
        Self { inner, reporter }
    }
}

impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buf)?;
        if count > 0 {
            self.reporter.advance(count as u64);
        }
        Ok(count)
    }
}

struct CountingWriter<W> {
    inner: W,
    written: u64,
}

impl<W> CountingWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, written: 0 }
    }

    fn written(&self) -> u64 {
        self.written
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(buf)?;
        self.written = self.written.saturating_add(count as u64);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

enum OutputSink {
    Plain(BufWriter<File>),
    Encrypted(EncryptedWriter<BufWriter<File>>),
    MultiVolume(MultiVolumeWriter),
    MultiVolumeEncrypted(EncryptedWriter<MultiVolumeWriter>),
}

impl OutputSink {
    /// Flush every layer *and* force the bytes to stable storage. Without the
    /// final `sync_all` we would report "done" while the archive still lives
    /// only in the page cache — a power loss then leaves a truncated file that
    /// looks complete.
    fn finalize(self) -> Result<()> {
        match self {
            Self::Plain(mut writer) => {
                writer.flush().context("刷新输出文件失败")?;
                writer
                    .get_ref()
                    .sync_all()
                    .context("同步输出文件到磁盘失败")?;
            }
            Self::Encrypted(writer) => {
                let mut inner = writer.finish().context("完成加密输出失败")?;
                inner.flush().context("刷新输出文件失败")?;
                inner
                    .get_ref()
                    .sync_all()
                    .context("同步输出文件到磁盘失败")?;
            }
            Self::MultiVolume(mut writer) => {
                writer.flush().context("刷新分卷输出失败")?;
                writer.sync_all().context("同步分卷输出到磁盘失败")?;
            }
            Self::MultiVolumeEncrypted(writer) => {
                let mut inner = writer.finish().context("完成分卷加密输出失败")?;
                inner.sync_all().context("同步分卷输出到磁盘失败")?;
            }
        }
        Ok(())
    }
}

impl Write for OutputSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(writer) => writer.write(buf),
            Self::Encrypted(writer) => writer.write(buf),
            Self::MultiVolume(writer) => writer.write(buf),
            Self::MultiVolumeEncrypted(writer) => writer.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(writer) => writer.flush(),
            Self::Encrypted(writer) => writer.flush(),
            Self::MultiVolume(writer) => writer.flush(),
            Self::MultiVolumeEncrypted(writer) => writer.flush(),
        }
    }
}

struct EncryptedWriter<W: Write> {
    inner: W,
    cipher: XChaCha20Poly1305,
    nonce_prefix: [u8; ENC_NONCE_PREFIX_LEN],
    counter: u64,
    /// Plaintext staging area, reused across chunks. Grows to
    /// `ENC_CHUNK_SIZE + ENC_TAG_LEN` once and is then encrypted in place.
    buffer: Vec<u8>,
    /// Bytes of `buffer` that hold pending plaintext.
    pending: usize,
    finished: bool,
}

impl<W: Write> EncryptedWriter<W> {
    fn new(mut inner: W, password: &str) -> Result<Self> {
        let mut salt = [0_u8; ENC_SALT_LEN];
        let mut nonce_prefix = [0_u8; ENC_NONCE_PREFIX_LEN];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce_prefix);

        let key = derive_encryption_key(password, &salt, CURRENT_KDF)?;
        let cipher = key.cipher();

        inner
            .write_all(ENC_MAGIC_V2)
            .context("写入加密头失败: magic")?;
        inner.write_all(&salt).context("写入加密头失败: salt")?;
        inner
            .write_all(&nonce_prefix)
            .context("写入加密头失败: nonce prefix")?;
        inner
            .write_all(&CURRENT_KDF.to_bytes())
            .context("写入加密头失败: kdf params")?;

        let mut buffer = Vec::new();
        buffer.reserve_exact(ENC_CHUNK_SIZE + ENC_TAG_LEN);

        Ok(Self {
            inner,
            cipher,
            nonce_prefix,
            counter: 0,
            buffer,
            pending: 0,
            finished: false,
        })
    }

    /// Encrypt `self.buffer[..self.pending]` in place (appending the AEAD tag)
    /// and emit it with its length prefix. No per-chunk heap allocation.
    fn flush_pending_chunk(&mut self) -> io::Result<()> {
        debug_assert!(self.pending > 0);
        let nonce = make_nonce(self.nonce_prefix, self.counter);
        self.counter = self.counter.saturating_add(1);

        self.buffer.truncate(self.pending);
        self.cipher
            .encrypt_in_place(XNonce::from_slice(&nonce), &[], &mut self.buffer)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "加密失败"))?;

        let len = self.buffer.len() as u32;
        self.inner.write_all(&len.to_be_bytes())?;
        self.inner.write_all(&self.buffer)?;

        self.buffer.clear();
        self.pending = 0;
        Ok(())
    }

    /// Write the terminator and hand the inner writer back so the caller can
    /// `fsync` it before reporting success.
    fn finish(mut self) -> io::Result<W> {
        if !self.finished {
            if self.pending > 0 {
                self.flush_pending_chunk()?;
            }

            self.inner.write_all(&0_u32.to_be_bytes())?;
            self.inner.flush()?;
            self.finished = true;
        }
        Ok(self.inner)
    }
}

impl<W: Write> Write for EncryptedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.finished {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "加密写入器已结束",
            ));
        }

        let mut consumed = 0;
        while consumed < buf.len() {
            let room = ENC_CHUNK_SIZE - self.pending;
            let take = room.min(buf.len() - consumed);
            self.buffer
                .extend_from_slice(&buf[consumed..consumed + take]);
            self.pending += take;
            consumed += take;

            if self.pending == ENC_CHUNK_SIZE {
                self.flush_pending_chunk()?;
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct EncryptedReader<R: Read> {
    inner: R,
    cipher: XChaCha20Poly1305,
    nonce_prefix: [u8; ENC_NONCE_PREFIX_LEN],
    counter: u64,
    decrypted: Vec<u8>,
    pos: usize,
    eof: bool,
}

impl<R: Read> EncryptedReader<R> {
    fn new(mut inner: R, password: &str) -> Result<Self> {
        let mut magic = [0_u8; 8];
        inner
            .read_exact(&mut magic)
            .context("读取加密头失败: magic")?;

        let versioned_v2 = &magic == ENC_MAGIC_V2;
        if !versioned_v2 && &magic != ENC_MAGIC_V1 {
            bail!("无效加密文件头，无法识别的归档格式");
        }

        let mut salt = [0_u8; ENC_SALT_LEN];
        let mut nonce_prefix = [0_u8; ENC_NONCE_PREFIX_LEN];
        inner
            .read_exact(&mut salt)
            .context("读取加密头失败: salt")?;
        inner
            .read_exact(&mut nonce_prefix)
            .context("读取加密头失败: nonce prefix")?;

        let kdf = if versioned_v2 {
            let mut raw = [0_u8; 12];
            inner
                .read_exact(&mut raw)
                .context("读取加密头失败: kdf params")?;
            KdfParams::from_bytes(raw)?
        } else {
            LEGACY_KDF
        };

        let key = derive_encryption_key(password, &salt, kdf)?;
        let cipher = key.cipher();

        Ok(Self {
            inner,
            cipher,
            nonce_prefix,
            counter: 0,
            decrypted: Vec::new(),
            pos: 0,
            eof: false,
        })
    }

    fn read_next_chunk(&mut self) -> io::Result<()> {
        if self.eof {
            return Ok(());
        }

        let mut len_buf = [0_u8; 4];
        self.inner.read_exact(&mut len_buf)?;
        let chunk_len = u32::from_be_bytes(len_buf) as usize;
        if chunk_len == 0 {
            self.eof = true;
            self.decrypted.clear();
            self.pos = 0;
            return Ok(());
        }

        // Reject implausible lengths *before* allocating: the writer never emits
        // more than one chunk plus its tag, so anything larger is corrupt or forged.
        if !(ENC_TAG_LEN..=ENC_MAX_CHUNK_LEN).contains(&chunk_len) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("加密分块长度非法({chunk_len} 字节)，文件已损坏或不是 ZARC 归档"),
            ));
        }

        // `decrypted` doubles as the ciphertext staging area; decryption happens in
        // place and shrinks it by exactly the tag length.
        self.decrypted.clear();
        self.decrypted.resize(chunk_len, 0);
        self.inner.read_exact(&mut self.decrypted)?;

        let nonce = make_nonce(self.nonce_prefix, self.counter);
        self.counter = self.counter.saturating_add(1);

        if self
            .cipher
            .decrypt_in_place(XNonce::from_slice(&nonce), &[], &mut self.decrypted)
            .is_err()
        {
            // Never leave unauthenticated bytes readable: a caller that ignores the
            // error and reads again must see EOF, not partially-decrypted garbage.
            self.decrypted.clear();
            self.pos = 0;
            self.eof = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "解密失败：密码错误或文件已损坏",
            ));
        }

        self.pos = 0;
        Ok(())
    }
}

impl<R: Read> Read for EncryptedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut written = 0_usize;
        while written < buf.len() {
            if self.pos >= self.decrypted.len() {
                self.read_next_chunk()?;
                if self.eof {
                    break;
                }
            }

            let available = self.decrypted.len().saturating_sub(self.pos);
            if available == 0 {
                break;
            }

            let take = (buf.len() - written).min(available);
            buf[written..written + take]
                .copy_from_slice(&self.decrypted[self.pos..self.pos + take]);
            self.pos += take;
            written += take;
        }

        Ok(written)
    }
}

#[tauri::command]
async fn compress_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CompressRequest,
) -> std::result::Result<OperationReport, String> {
    state.reset_abort();
    let state_inner = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        compress_archive_sync(request, Some(app), Some(state_inner))
    })
    .await
    .map_err(|err| format!("任务线程异常: {err}"))?
    .map_err(|err| err.to_string())
}

#[tauri::command]
async fn decompress_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    request: DecompressRequest,
) -> std::result::Result<OperationReport, String> {
    state.reset_abort();
    let state_inner = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        decompress_archive_sync(request, Some(app), Some(state_inner))
    })
    .await
    .map_err(|err| format!("任务线程异常: {err}"))?
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_embedded_archive_info() -> std::result::Result<Option<EmbeddedArchiveInfo>, String> {
    sfx::load_embedded_archive_info_from_current_exe().map_err(|err| err.to_string())
}

/// Walk `root` and total up file sizes, stopping early once the walk becomes
/// expensive enough that the UI would rather show an estimate than block.
fn measure_directory(root: &Path) -> (u64, u64, bool) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    let walk_root = fs_access_path(root).unwrap_or_else(|_| root.to_path_buf());
    for entry in WalkDir::new(&walk_root)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if files >= PATH_INSPECT_ENTRY_CAP {
            return (bytes, files, true);
        }
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                bytes = bytes.saturating_add(metadata.len());
            }
            files += 1;
        }
    }
    (bytes, files, false)
}

/// Ask the filesystem what a path actually is.
///
/// The frontend used to guess with `basename.includes('.')`, which labels
/// `release.v2/` a file and `Makefile` a directory — and then hands the wrong
/// `include_root_dir` semantics to the backend. Only a `stat` can answer this.
#[tauri::command]
async fn inspect_path(path: String) -> std::result::Result<PathInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target = PathBuf::from(&path);
        let Ok(access_target) = fs_access_path(&target) else {
            return PathInfo {
                path,
                exists: false,
                is_dir: false,
                size_bytes: 0,
                file_count: 0,
                truncated: false,
            };
        };
        let Ok(metadata) = fs::metadata(&access_target) else {
            return PathInfo {
                path,
                exists: false,
                is_dir: false,
                size_bytes: 0,
                file_count: 0,
                truncated: false,
            };
        };

        if metadata.is_dir() {
            let (size_bytes, file_count, truncated) = measure_directory(&target);
            PathInfo { path, exists: true, is_dir: true, size_bytes, file_count, truncated }
        } else {
            PathInfo {
                path,
                exists: true,
                is_dir: false,
                size_bytes: metadata.len(),
                file_count: 1,
                truncated: false,
            }
        }
    })
    .await
    .map_err(|err| format!("路径检查线程异常: {err}"))
}

#[tauri::command]
async fn extract_embedded_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmbeddedDecompressRequest,
) -> std::result::Result<OperationReport, String> {
    state.reset_abort();
    let state_inner = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        sfx::extract_embedded_archive_from_current_exe(request, Some(app), Some(state_inner))
    })
    .await
    .map_err(|err| format!("任务线程异常: {err}"))?
    .map_err(|err| err.to_string())
}

#[tauri::command]
async fn benchmark_compression(
    state: State<'_, AppState>,
    request: BenchmarkRequest,
) -> std::result::Result<BenchmarkReport, String> {
    state.reset_abort();
    let state_inner = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        benchmark_compression_sync(request, Some(state_inner))
    })
    .await
    .map_err(|err| format!("任务线程异常: {err}"))?
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn abort_task(state: State<'_, AppState>) {
    state.request_abort();
}

/// Log sink, opened at most once per process.
///
/// The previous implementation re-opened `zarc.log` for every line, so logging
/// cost a syscall pair per message and silently did nothing whenever the app
/// lived in a read-only directory (`/Applications`, `Program Files`). Now the
/// handle is cached and we fall back to the temp dir when the exe directory is
/// not writable.
static LOG_FILE: OnceLock<Option<Mutex<File>>> = OnceLock::new();

fn log_sink() -> Option<&'static Mutex<File>> {
    LOG_FILE
        .get_or_init(|| {
            let mut candidates = Vec::new();
            if let Ok(mut path) = std::env::current_exe() {
                path.pop();
                candidates.push(path.join("zarc.log"));
            }
            candidates.push(std::env::temp_dir().join("zarc.log"));

            candidates.into_iter().find_map(|path| {
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .ok()
                    .map(Mutex::new)
            })
        })
        .as_ref()
}

fn log_to_file(enabled: bool, message: &str) {
    if !enabled {
        return;
    }
    if let Some(file) = log_sink() {
        if let Ok(mut file) = file.lock() {
            let _ = writeln!(file, "{message}");
            let _ = file.flush();
        }
    }
}

fn compress_archive_sync(
    request: CompressRequest,
    app: Option<AppHandle>,
    state: Option<AppState>,
) -> Result<OperationReport> {
    let source = PathBuf::from(request.source_path.trim());
    let source_access = fs_access_path(&source)
        .with_context(|| format!("无法准备源路径: {}", source.display()))?;
    let source_metadata = fs::metadata(&source_access)
        .with_context(|| format!("源路径不存在或无法访问: {}", source.display()))?;

    let output_kind = OutputKind::archive_or_default(request.output_kind);
    let level = request.level.unwrap_or(8).clamp(1, 22);
    let include_root_dir = request.include_root_dir.unwrap_or(true);
    let password = normalize_password(request.password.clone());
    let split_size_mib = request.split_size_mib;
    let enable_logging = request.enable_logging.unwrap_or(false);
    let delete_source_after = request.delete_source_after.unwrap_or(false);
    let request_threads = request.threads;

    let source_bytes = count_source_bytes(&source)?;
    let output = resolve_compress_output(
        &source,
        request.output_path.as_deref(),
        password.is_some(),
        output_kind,
    )?;
    validate_compress_paths(&source, &output, output_kind, split_size_mib)?;

    log_to_file(
        enable_logging,
        &format!("开始压缩: {} -> {}", source.display(), output.display()),
    );

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("无法创建输出目录: {}", parent.display()))?;
    }

    let reporter = ProgressReporter::new(app, "compress", source_bytes);
    reporter.begin();

    if output_kind == OutputKind::SfxExe {
        if split_size_mib.unwrap_or(0) > 0 {
            let err = anyhow!("Windows 自解压 EXE 暂不支持分卷");
            reporter.fail(err.to_string());
            return Err(err);
        }

        let mut sfx_request = request;
        sfx_request.output_kind = Some(output_kind);
        sfx_request.output_path = Some(path_to_string(&output));
        return sfx::compress_sfx_archive_sync(
            sfx_request,
            output,
            enable_logging,
            delete_source_after,
            reporter,
            state,
            source_bytes,
        );
    }

    let started = Instant::now();
    let operation_result = if source_metadata.is_dir() {
        compress_directory(
            &source,
            &output,
            level,
            include_root_dir,
            password.as_deref(),
            &reporter,
            state.as_ref(),
            split_size_mib,
            request_threads,
        )
    } else {
        compress_file(
            &source,
            &output,
            level,
            password.as_deref(),
            &reporter,
            state.as_ref(),
            split_size_mib,
            request_threads,
        )
    };

    if let Err(err) = operation_result {
        cleanup_compress_output(&output, split_size_mib);
        reporter.fail(err.to_string());
        log_to_file(enable_logging, &format!("压缩失败: {}", err));
        return Err(err);
    }

    let duration = started.elapsed().as_secs_f64();
    let (reported_output, output_bytes, hash) = compress_output_report(&output, split_size_mib)?;

    log_to_file(
        enable_logging,
        &format!(
            "压缩完成. 原始大小: {}, 压缩后: {}, 耗时: {:.2}s",
            source_bytes, output_bytes, duration
        ),
    );

    if delete_source_after {
        log_to_file(enable_logging, &format!("正在删除源: {}", source.display()));
        if let Err(err) = delete_source_path(&source) {
            reporter.fail(err.to_string());
            log_to_file(enable_logging, &format!("删除源失败: {}", err));
            return Err(err);
        }
    }

    reporter.finish();

    Ok(OperationReport {
        operation: "compress".to_string(),
        source_path: path_to_string(&source),
        output_path: path_to_string(&reported_output),
        source_bytes,
        output_bytes,
        duration_ms: duration * 1000.0,
        throughput_mi_bs: throughput(source_bytes, duration),
        compression_ratio: Some(ratio(output_bytes, source_bytes)),
        blake3_hash: hash,
        sidecar_path: None,
    })
}

fn decompress_archive_sync(
    request: DecompressRequest,
    app: Option<AppHandle>,
    state: Option<AppState>,
) -> Result<OperationReport> {
    let archive = PathBuf::from(request.archive_path.trim());
    if !archive.exists() {
        bail!("归档文件不存在: {}", archive.display());
    }

    let meta = detect_archive_meta(&archive)?;
    let password = normalize_password(request.password);
    if meta.encrypted && password.is_none() {
        bail!("该归档已加密，请提供解密密码");
    }

    let source_bytes = archive_input_bytes(&archive, meta)?;

    let output = resolve_decompress_output(&archive, meta, request.output_path.as_deref())?;
    validate_decompress_paths(&archive, &output, meta)?;

    let parent = output_parent(&output);
    fs::create_dir_all(parent)
        .with_context(|| format!("无法创建输出目录: {}", parent.display()))?;

    let reporter = ProgressReporter::new(app, "decompress", source_bytes);
    reporter.begin();

    let started = Instant::now();

    let reader: Box<dyn Read> = if meta.is_multi_volume {
        Box::new(MultiVolumeReader::new(archive.clone()))
    } else {
        Box::new(
            File::open(&archive)
                .with_context(|| format!("无法打开归档文件: {}", archive.display()))?,
        )
    };

    let buf_reader = BufReader::with_capacity(IO_BUFFER_SIZE, reader);
    let progress_reader = ProgressReader::new(buf_reader, reporter.clone());

    let output_result = if meta.encrypted {
        let decrypt_reader =
            EncryptedReader::new(progress_reader, password.as_deref().unwrap_or_default())?;
        decompress_reader_transactionally(decrypt_reader, meta.kind, &output, state.as_ref())
    } else {
        decompress_reader_transactionally(progress_reader, meta.kind, &output, state.as_ref())
    };

    let output_bytes = match output_result {
        Ok(bytes) => bytes,
        Err(err) => {
            reporter.fail(err.to_string());
            return Err(err);
        }
    };

    reporter.finish();

    let duration = started.elapsed().as_secs_f64();
    let hash = calculate_archive_hash(&archive, meta).ok();

    Ok(OperationReport {
        operation: "decompress".to_string(),
        source_path: path_to_string(&archive),
        output_path: path_to_string(&output),
        source_bytes,
        output_bytes,
        duration_ms: duration * 1000.0,
        throughput_mi_bs: throughput(output_bytes.max(source_bytes), duration),
        compression_ratio: None,
        blake3_hash: hash,
        sidecar_path: None,
    })
}

fn calculate_file_hash(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(IO_BUFFER_SIZE, file);
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn benchmark_compression_sync(
    request: BenchmarkRequest,
    state: Option<AppState>,
) -> Result<BenchmarkReport> {
    let source = PathBuf::from(request.source_path.trim());
    if !source.exists() {
        bail!("源路径不存在: {}", source.display());
    }

    let mut min_level = request.min_level.unwrap_or(1).clamp(1, 22);
    let mut max_level = request.max_level.unwrap_or(12).clamp(1, 22);
    if min_level > max_level {
        std::mem::swap(&mut min_level, &mut max_level);
    }

    let iterations = request.iterations.unwrap_or(2).clamp(1, 12);
    let sample_size_mib = request.sample_size_mib.unwrap_or(64).clamp(4, 1024);
    let sample_limit = sample_size_mib as usize * 1024 * 1024;

    let threads = resolve_threads(request.threads);

    let sample = load_benchmark_sample(&source, sample_limit)?;
    if sample.is_empty() {
        bail!("基准测试样本为空，无法评估压缩等级");
    }

    let sample_bytes = sample.len() as u64;
    let mut results = Vec::new();

    for level in min_level..=max_level {
        if let Some(s) = &state {
            if s.is_aborted() {
                bail!("用户已终止测试");
            }
        }

        let mut ms_samples = Vec::with_capacity(iterations as usize);
        let mut throughput_samples = Vec::with_capacity(iterations as usize);
        let mut compressed_bytes = 0_u64;

        for _ in 0..iterations {
            if let Some(s) = &state {
                if s.is_aborted() {
                    bail!("用户已终止测试");
                }
            }

            let start = Instant::now();
            compressed_bytes = compress_to_count(&sample, level as i32, threads, state.as_ref())?;
            let elapsed = start.elapsed().as_secs_f64();

            ms_samples.push(elapsed * 1000.0);
            throughput_samples.push(throughput(sample_bytes, elapsed));
        }

        results.push(CompressionLevelReport {
            level,
            mean_ms: mean(&ms_samples),
            mean_throughput_mi_bs: mean(&throughput_samples),
            compressed_bytes,
            ratio_percent: ratio(compressed_bytes, sample_bytes),
            score: 0.0,
        });
    }

    apply_score(&mut results);
    let recommended_level = choose_recommended_level(&results)
        .with_context(|| "无法从 benchmark 结果中推导推荐等级")?;

    let note = format!(
        "基于样本大小约 {:.2} MiB 的快速压缩测试。推荐等级平衡了压缩率与吞吐（权重：率 60%，速度 40%）。",
        sample_bytes as f64 / MIB
    );

    Ok(BenchmarkReport {
        source_path: path_to_string(&source),
        sample_bytes,
        min_level,
        max_level,
        iterations,
        threads,
        recommended_level,
        results,
        note,
    })
}

fn derive_encryption_key(
    password: &str,
    salt: &[u8; ENC_SALT_LEN],
    kdf: KdfParams,
) -> Result<SecretKey> {
    let mut key = SecretKey([0_u8; ENC_KEY_LEN]);

    let params = Params::new(
        kdf.m_cost_kib,
        kdf.t_cost,
        kdf.parallelism,
        Some(ENC_KEY_LEN),
    )
    .map_err(|err| anyhow!("创建 Argon2 参数失败: {err}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key.0)
        .map_err(|err| anyhow!("密码派生失败: {err}"))?;

    Ok(key)
}

fn make_nonce(prefix: [u8; ENC_NONCE_PREFIX_LEN], counter: u64) -> [u8; 24] {
    let mut nonce = [0_u8; 24];
    nonce[..ENC_NONCE_PREFIX_LEN].copy_from_slice(&prefix);
    nonce[ENC_NONCE_PREFIX_LEN..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

fn create_output_sink(
    path: &Path,
    password: Option<&str>,
    split_size_mib: Option<u64>,
) -> Result<OutputSink> {
    match (password, split_size_mib) {
        (Some(pwd), Some(size)) if size > 0 => {
            let writer = MultiVolumeWriter::new(path.to_path_buf(), size);
            Ok(OutputSink::MultiVolumeEncrypted(EncryptedWriter::new(
                writer, pwd,
            )?))
        }
        (None, Some(size)) if size > 0 => Ok(OutputSink::MultiVolume(MultiVolumeWriter::new(
            path.to_path_buf(),
            size,
        ))),
        (Some(pwd), _) => {
            let file = File::create(path)
                .with_context(|| format!("无法创建输出文件: {}", path.display()))?;
            let writer = BufWriter::with_capacity(IO_BUFFER_SIZE, file);
            Ok(OutputSink::Encrypted(EncryptedWriter::new(writer, pwd)?))
        }
        (None, _) => {
            let file = File::create(path)
                .with_context(|| format!("无法创建输出文件: {}", path.display()))?;
            let writer = BufWriter::with_capacity(IO_BUFFER_SIZE, file);
            Ok(OutputSink::Plain(writer))
        }
    }
}

struct MultiVolumeReader {
    base_path: PathBuf,
    current_file: Option<BufReader<File>>,
    current_index: usize,
}

impl MultiVolumeReader {
    fn new(any_volume_path: PathBuf) -> Self {
        Self {
            base_path: volume_base_path(&any_volume_path),
            current_file: None,
            current_index: 1,
        }
    }

    fn open_next(&mut self) -> io::Result<bool> {
        let ext = format!("{:03}", self.current_index);
        let mut path = self.base_path.clone();
        let base_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        path.set_file_name(format!("{base_name}.{ext}"));

        if path.exists() {
            let file = File::open(path)?;
            self.current_file = Some(BufReader::with_capacity(IO_BUFFER_SIZE, file));
            self.current_index += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Read for MultiVolumeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.current_file.is_none() && !self.open_next()? {
                return Ok(0);
            }

            let n = self.current_file.as_mut().unwrap().read(buf)?;
            if n > 0 {
                return Ok(n);
            } else {
                self.current_file = None;
            }
        }
    }
}

fn multi_volume_path(base_path: &Path, index: usize) -> Result<PathBuf> {
    let file_name = base_path
        .file_name()
        .with_context(|| format!("分卷输出路径缺少文件名: {}", base_path.display()))?
        .to_string_lossy();
    let mut path = base_path.to_path_buf();
    path.set_file_name(format!("{file_name}.{index:03}"));
    Ok(path)
}

fn split_enabled(split_size_mib: Option<u64>) -> bool {
    split_size_mib.unwrap_or(0) > 0
}

/// Strip a trailing `.NNN` volume suffix — *any* index, not just `.001`. Users
/// routinely drop `archive.tar.zst.003` onto the window; resolving the base name
/// from it is what makes "选择任意分卷" work instead of erroring out.
fn volume_base_path(volume_path: &Path) -> PathBuf {
    let mut base = volume_path.to_path_buf();
    let Some(name) = volume_path.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return base;
    };
    if let Some((stem, suffix)) = name.rsplit_once('.') {
        if !stem.is_empty() && is_volume_suffix(suffix) {
            base.set_file_name(stem);
        }
    }
    base
}

fn is_volume_suffix(suffix: &str) -> bool {
    suffix.len() >= 3 && suffix.bytes().all(|b| b.is_ascii_digit())
}

/// Indices of every `base.NNN` sibling that exists, ascending.
///
/// A directory scan, not a `1, 2, 3, …` probe. Probing stops at the first miss,
/// which silently hides a *gap*: with only `.001` and `.003` present it reports
/// a one-volume archive and decompresses a third of the data, surfacing much
/// later as an unrelated zstd error.
fn volume_indices(base_path: &Path) -> Result<BTreeSet<usize>> {
    let dir = output_parent(base_path);
    let base_name = base_path
        .file_name()
        .with_context(|| format!("分卷路径缺少文件名: {}", base_path.display()))?
        .to_string_lossy()
        .to_string();
    let prefix = format!("{base_name}.");

    let mut indices = BTreeSet::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        // Nothing can exist inside a directory that doesn't exist yet.
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(indices),
        Err(err) => {
            return Err(err).with_context(|| format!("无法读取分卷所在目录: {}", dir.display()))
        }
    };

    for entry in entries {
        let entry = entry.with_context(|| format!("读取目录项失败: {}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(suffix) = name.strip_prefix(&prefix) else {
            continue;
        };
        if !is_volume_suffix(suffix) {
            continue;
        }
        if let Ok(index) = suffix.parse::<usize>() {
            if index >= 1 {
                indices.insert(index);
            }
        }
    }
    Ok(indices)
}

/// Resolve the full `base.001 .. base.NNN` chain, naming any missing volume.
fn validate_volume_chain(base_path: &Path) -> Result<Vec<PathBuf>> {
    let indices = volume_indices(base_path)?;
    if !indices.contains(&1) {
        bail!(
            "未找到分卷归档首卷: {}",
            multi_volume_path(base_path, 1)?.display()
        );
    }

    let last = *indices.iter().next_back().expect("chain is non-empty");
    let mut volumes = Vec::with_capacity(last);
    for index in 1..=last {
        let path = multi_volume_path(base_path, index)?;
        if !indices.contains(&index) {
            bail!("分卷归档不完整，缺少第 {index} 卷: {}", path.display());
        }
        volumes.push(path);
    }
    Ok(volumes)
}

/// Every existing volume of `base_path`, ascending. Gaps are reported as they
/// are — use [`validate_volume_chain`] when contiguity matters.
fn existing_volume_paths(base_path: &Path) -> Result<Vec<PathBuf>> {
    volume_indices(base_path)?
        .into_iter()
        .map(|index| multi_volume_path(base_path, index))
        .collect()
}

fn cleanup_compress_output(base_path: &Path, split_size_mib: Option<u64>) {
    if split_enabled(split_size_mib) {
        if let Ok(paths) = existing_volume_paths(base_path) {
            for path in paths {
                let _ = fs::remove_file(path);
            }
        }
    } else {
        let _ = fs::remove_file(base_path);
    }
}

fn compress_output_report(
    base_path: &Path,
    split_size_mib: Option<u64>,
) -> Result<(PathBuf, u64, Option<String>)> {
    if !split_enabled(split_size_mib) {
        let output_bytes = fs::metadata(base_path)
            .with_context(|| format!("无法读取结果文件信息: {}", base_path.display()))?
            .len();
        return Ok((
            base_path.to_path_buf(),
            output_bytes,
            calculate_file_hash(base_path).ok(),
        ));
    }

    let volumes = existing_volume_paths(base_path)?;
    if volumes.is_empty() {
        bail!(
            "未找到分卷输出首卷: {}",
            multi_volume_path(base_path, 1)?.display()
        );
    }

    let mut output_bytes = 0_u64;
    for volume in &volumes {
        output_bytes = output_bytes.saturating_add(
            fs::metadata(volume)
                .with_context(|| format!("无法读取分卷文件信息: {}", volume.display()))?
                .len(),
        );
    }

    Ok((volumes[0].clone(), output_bytes, None))
}

fn archive_input_bytes(archive: &Path, meta: ArchiveMeta) -> Result<u64> {
    let mut total = 0_u64;
    for volume in archive_volume_paths(archive, meta)? {
        total = total.saturating_add(
            fs::metadata(&volume)
                .with_context(|| format!("无法读取归档信息: {}", volume.display()))?
                .len(),
        );
    }
    Ok(total)
}

/// Every file that makes up `archive` — the single file itself, or the complete
/// validated volume chain.
fn archive_volume_paths(archive: &Path, meta: ArchiveMeta) -> Result<Vec<PathBuf>> {
    if !meta.is_multi_volume {
        return Ok(vec![archive.to_path_buf()]);
    }
    validate_volume_chain(&volume_base_path(archive))
}

/// Hash the archive *as a whole*. For a split archive that means every volume in
/// order; hashing only `.001` reported a digest that could never be reproduced
/// from the reassembled file.
fn calculate_archive_hash(archive: &Path, meta: ArchiveMeta) -> Result<String> {
    if !meta.is_multi_volume {
        return calculate_file_hash(archive);
    }

    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    for volume in archive_volume_paths(archive, meta)? {
        let mut file = File::open(&volume)
            .with_context(|| format!("无法打开分卷归档: {}", volume.display()))?;
        loop {
            let read = file
                .read(&mut buffer)
                .with_context(|| format!("读取分卷归档失败: {}", volume.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn delete_source_path(source: &Path) -> Result<()> {
    if source.is_dir() {
        fs::remove_dir_all(source).with_context(|| format!("删除源目录失败: {}", source.display()))
    } else {
        fs::remove_file(source).with_context(|| format!("删除源文件失败: {}", source.display()))
    }
}

/// Bytes fed to zstd per `write_all` during a benchmark pass. Small enough that
/// an abort request is honoured promptly even at level 22 on a large sample.
const BENCH_FEED_CHUNK: usize = 4 * 1024 * 1024;

fn compress_to_count(
    data: &[u8],
    level: i32,
    threads: u32,
    state: Option<&AppState>,
) -> Result<u64> {
    let sink = CountingWriter::new(io::sink());
    let mut encoder = zstd::Encoder::new(sink, level).context("创建 zstd 编码器失败")?;
    encoder
        .multithread(threads)
        .context("无法开启 zstd 多线程压缩")?;

    // One `write_all(data)` for the whole sample meant "停止" was ignored until
    // the level finished — tens of seconds at level 22. Feed it in chunks and
    // check the abort flag between them.
    for chunk in data.chunks(BENCH_FEED_CHUNK.max(1)) {
        if let Some(s) = state {
            if s.is_aborted() {
                bail!("用户已终止任务");
            }
        }
        encoder
            .write_all(chunk)
            .context("写入压缩样本失败，无法完成快速测试")?;
    }

    let mut sink = encoder.finish().context("无法完成压缩编码")?;
    sink.flush().context("刷新压缩输出失败")?;

    Ok(sink.written())
}

fn load_benchmark_sample(source: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let mut sample = Vec::new();
    if max_bytes == 0 {
        return Ok(sample);
    }

    if source.is_file() {
        let mut file = File::open(source)
            .with_context(|| format!("无法读取基准测试源文件: {}", source.display()))?;
        let total_size = file.metadata()?.len() as usize;

        if total_size <= max_bytes {
            file.read_to_end(&mut sample)?;
        } else {
            // Sample from beginning, middle, and end
            let chunk_size = max_bytes / 3;

            // Beginning
            read_chunk(&mut file, 0, chunk_size, &mut sample)?;

            // Middle
            read_chunk(
                &mut file,
                (total_size / 2).saturating_sub(chunk_size / 2),
                chunk_size,
                &mut sample,
            )?;

            // End
            read_chunk(
                &mut file,
                total_size.saturating_sub(chunk_size),
                chunk_size,
                &mut sample,
            )?;
        }
        return Ok(sample);
    }

    for entry in WalkDir::new(source)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path();
        let file = File::open(file_path)
            .with_context(|| format!("无法读取目录样本文件: {}", file_path.display()))?;

        let remaining = max_bytes.saturating_sub(sample.len());
        if remaining == 0 {
            break;
        }

        // `read` may return a short count for any reason; `take(..).read_to_end`
        // keeps pulling until the cap or real EOF, so the sample size actually
        // reflects what was asked for.
        let want = remaining.min(1024 * 1024) as u64;
        file.take(want)
            .read_to_end(&mut sample)
            .with_context(|| format!("读取目录样本文件失败: {}", file_path.display()))?;

        if sample.len() >= max_bytes {
            break;
        }
    }

    Ok(sample)
}

fn read_chunk(file: &mut File, offset: usize, size: usize, sample: &mut Vec<u8>) -> Result<()> {
    use std::io::Seek;
    file.seek(io::SeekFrom::Start(offset as u64))?;
    // Same reasoning as above: a single `read` could hand back a few bytes and
    // silently shrink the benchmark sample to a fraction of the requested size.
    file.take(size as u64)
        .read_to_end(sample)
        .context("读取基准测试样本失败")?;
    Ok(())
}

fn apply_score(results: &mut [CompressionLevelReport]) {
    if results.is_empty() {
        return;
    }

    let max_throughput = results
        .iter()
        .map(|item| item.mean_throughput_mi_bs)
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);

    let min_ratio = results
        .iter()
        .map(|item| item.ratio_percent)
        .fold(f64::INFINITY, f64::min)
        .max(f64::EPSILON);

    for item in results.iter_mut() {
        let speed_score = item.mean_throughput_mi_bs / max_throughput;
        let ratio_score = min_ratio / item.ratio_percent.max(f64::EPSILON);
        // Weight: 60% for compression ratio, 40% for speed
        item.score = speed_score * 0.40 + ratio_score * 0.60;
    }
}

fn choose_recommended_level(results: &[CompressionLevelReport]) -> Option<u8> {
    let mut iter = results.iter();
    let mut best = iter.next()?;

    for item in iter {
        let better_score = item.score > best.score + 1e-9;
        let same_score = (item.score - best.score).abs() <= 1e-9;
        let better_level = item.level < best.level;

        if better_score || (same_score && better_level) {
            best = item;
        }
    }

    Some(best.level)
}

fn compress_file(
    source: &Path,
    output: &Path,
    level: i32,
    password: Option<&str>,
    reporter: &ProgressReporter,
    state: Option<&AppState>,
    split_size_mib: Option<u64>,
    threads: Option<u32>,
) -> Result<()> {
    let access_path = fs_access_path(source)
        .with_context(|| format!("无法准备源文件路径: {}", source.display()))?;
    let input = File::open(&access_path)
        .with_context(|| format!("无法打开源文件: {}", source.display()))?;
    let mut reader = BufReader::with_capacity(IO_BUFFER_SIZE, input);

    let output_sink = create_output_sink(output, password, split_size_mib)?;
    let mut encoder = zstd::Encoder::new(output_sink, level).context("创建 zstd 编码器失败")?;

    encoder
        .multithread(resolve_threads(threads))
        .context("无法开启 zstd 多线程压缩")?;

    let mut buf = vec![0_u8; 512 * 1024];
    loop {
        if let Some(s) = state {
            if s.is_aborted() {
                bail!("用户已终止任务");
            }
        }

        let count = reader.read(&mut buf).context("读取压缩源文件失败")?;
        if count == 0 {
            break;
        }

        encoder
            .write_all(&buf[..count])
            .context("压缩过程中写入失败")?;
        reporter.advance(count as u64);
    }

    let sink = encoder.finish().context("无法完成压缩输出")?;
    sink.finalize()?;

    Ok(())
}

fn compress_directory(
    source: &Path,
    output: &Path,
    level: i32,
    include_root_dir: bool,
    password: Option<&str>,
    reporter: &ProgressReporter,
    state: Option<&AppState>,
    split_size_mib: Option<u64>,
    threads: Option<u32>,
) -> Result<()> {
    let output_sink = create_output_sink(output, password, split_size_mib)?;
    let mut encoder = zstd::Encoder::new(output_sink, level).context("创建 zstd 编码器失败")?;

    encoder
        .multithread(resolve_threads(threads))
        .context("无法开启 zstd 多线程压缩")?;

    let mut tar_builder = tar::Builder::new(encoder);
    let root_name = source
        .file_name()
        .map(|v| v.to_owned())
        .with_context(|| format!("目录名称无效: {}", source.display()))?;
    // Keep every filesystem operation below in the same namespace. This also
    // lets a source directory itself have a DOS-reserved name on Windows.
    let walk_source = fs_access_path(source)
        .with_context(|| format!("无法准备源目录路径: {}", source.display()))?;

    if include_root_dir {
        tar_builder
            .append_dir(Path::new(&root_name), &walk_source)
            .with_context(|| format!("写入根目录失败: {}", source.display()))?;
    }

    // Windows treats names such as `NUL`, `CON`, `AUX`, `COM1`... as DOS
    // devices when they are reached through a normal drive path. Such entries
    // can still exist on NTFS (for example after being created by another OS or
    // sync tool). Walking through the verbatim namespace keeps those names as
    // literal filesystem entries instead of silently turning `...\NUL` into
    // the null device.
    for entry in WalkDir::new(&walk_source)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if let Some(s) = state {
            if s.is_aborted() {
                bail!("用户已终止任务");
            }
        }

        let path = entry.path();
        let rel = path
            .strip_prefix(&walk_source)
            .with_context(|| format!("无法计算相对路径: {}", path.display()))?;

        let archive_name = if include_root_dir {
            Path::new(&root_name).join(rel)
        } else {
            rel.to_path_buf()
        };

        if entry.file_type().is_dir() {
            tar_builder
                .append_dir(&archive_name, path)
                .with_context(|| format!("写入目录失败: {}", path.display()))?;
            continue;
        }

        // Symlinks used to be dropped silently: neither branch below matched
        // them, so a directory tree round-tripped through ZARC came back with
        // its links missing and no warning anywhere.
        if entry.file_type().is_symlink() {
            let access_path = fs_access_path(path)
                .with_context(|| format!("无法准备符号链接路径: {}", path.display()))?;
            let target = fs::read_link(&access_path)
                .with_context(|| format!("无法读取符号链接目标: {}", path.display()))?;
            let mut header = tar::Header::new_gnu();
            if let Ok(metadata) = fs::symlink_metadata(&access_path) {
                header.set_metadata(&metadata);
            }
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            tar_builder
                .append_link(&mut header, &archive_name, &target)
                .with_context(|| format!("写入符号链接失败: {}", path.display()))?;
            continue;
        }

        if entry.file_type().is_file() {
            append_file_with_progress(&mut tar_builder, path, &archive_name, reporter, state)?;
        }
    }

    tar_builder.finish().context("tar 归档收尾失败")?;
    let encoder = tar_builder.into_inner().context("无法获取压缩编码器")?;
    let sink = encoder.finish().context("无法完成目录压缩输出")?;
    sink.finalize()?;

    Ok(())
}

fn append_file_with_progress<W: Write>(
    tar_builder: &mut tar::Builder<W>,
    source_path: &Path,
    archive_name: &Path,
    reporter: &ProgressReporter,
    state: Option<&AppState>,
) -> Result<()> {
    let access_path = fs_access_path(source_path)
        .with_context(|| format!("无法准备文件路径: {}", source_path.display()))?;
    let file = File::open(&access_path)
        .with_context(|| format!("无法读取待归档文件: {}", source_path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("无法读取文件元数据: {}", source_path.display()))?;

    let mut header = tar::Header::new_gnu();
    header.set_metadata(&metadata);
    header.set_cksum();

    let reader = BufReader::with_capacity(IO_BUFFER_SIZE, file);
    let mut progress_reader = ProgressReader::new(reader, reporter.clone());

    // We can't easily check for abort inside tar_builder.append_data without a custom reader that checks state.
    // But ProgressReader is already there! Let's update ProgressReader.

    tar_builder
        .append_data(&mut header, archive_name, &mut progress_reader)
        .with_context(|| format!("写入文件失败: {}", source_path.display()))?;

    if let Some(s) = state {
        if s.is_aborted() {
            bail!("用户已终止任务");
        }
    }

    Ok(())
}

struct AbortableReader<R: Read> {
    inner: R,
    state: Option<AppState>,
}

impl<R: Read> AbortableReader<R> {
    fn new(inner: R, state: Option<&AppState>) -> Self {
        Self {
            inner,
            state: state.cloned(),
        }
    }
}

impl<R: Read> Read for AbortableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(s) = &self.state {
            if s.is_aborted() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "任务已终止"));
            }
        }
        self.inner.read(buf)
    }
}

fn decompress_tar_from_reader<R: Read>(
    reader: R,
    output_dir: &Path,
    state: Option<&AppState>,
) -> Result<()> {
    let decoder = zstd::Decoder::new(reader).context("创建 zstd 解码器失败")?;
    let abortable = AbortableReader::new(decoder, state);
    let mut archive = tar::Archive::new(abortable);
    let output_access = fs_access_path(output_dir)
        .with_context(|| format!("无法准备解压目录路径: {}", output_dir.display()))?;
    archive
        .unpack(&output_access)
        .with_context(|| format!("解包归档失败: {}", output_dir.display()))?;
    Ok(())
}

fn decompress_file_from_reader<R: Read>(
    reader: R,
    output_file: &Path,
    state: Option<&AppState>,
) -> Result<u64> {
    let mut decoder = zstd::Decoder::new(reader).context("创建 zstd 解码器失败")?;

    let output_access = fs_access_path(output_file)
        .with_context(|| format!("无法准备输出文件路径: {}", output_file.display()))?;
    let output = File::create(&output_access)
        .with_context(|| format!("无法创建输出文件: {}", output_file.display()))?;
    let mut writer = BufWriter::with_capacity(IO_BUFFER_SIZE, output);

    let mut output_bytes = 0_u64;
    let mut buffer = vec![0_u8; 512 * 1024];
    loop {
        if let Some(s) = state {
            if s.is_aborted() {
                bail!("用户已终止任务");
            }
        }

        let count = decoder.read(&mut buffer).context("解压读取失败")?;
        if count == 0 {
            break;
        }

        writer
            .write_all(&buffer[..count])
            .context("写入解压输出失败")?;
        output_bytes = output_bytes.saturating_add(count as u64);
    }

    writer.flush().context("解压结果刷盘失败")?;
    writer
        .get_ref()
        .sync_all()
        .context("同步解压结果到磁盘失败")?;

    Ok(output_bytes)
}

/// Serial number for staging paths, so two extractions in the same directory
/// within one process cannot pick the same name.
static TEMP_STAGING_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Reserve an unused `.zarc-tmp-<pid>-<seq>` staging path under `parent`.
///
/// The old name was `.zarc-tmp-<pid>` — shared by every concurrent extraction in
/// that directory, and the second one *deleted* the first one's staging area
/// before starting.
fn unique_temp_path(parent: &Path) -> Result<PathBuf> {
    let pid = std::process::id();
    for _ in 0..1024 {
        let seq = TEMP_STAGING_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        let candidate = parent.join(format!(".zarc-tmp-{pid}-{seq}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("无法在 {} 下分配临时解压路径", parent.display());
}

/// Deletes a staging path on drop unless disarmed.
///
/// Hand-rolled cleanup did not survive contact with `?`: a `?` inside a match
/// arm returned straight out of the function and skipped the cleanup arm
/// written below it, leaving a full partially-extracted tree behind on every
/// failed extraction.
struct TempPathGuard {
    path: PathBuf,
    armed: bool,
}

impl TempPathGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempPathGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if self.path.is_dir() {
            let _ = fs::remove_dir_all(&self.path);
        } else {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn decompress_reader_transactionally<R: Read>(
    reader: R,
    kind: ArchiveKind,
    output: &Path,
    state: Option<&AppState>,
) -> Result<u64> {
    // Checked up front too: no point decompressing 50 GiB before discovering we
    // are not allowed to place it. Re-checked after staging to cover the window
    // in between.
    if output.exists() {
        bail!("输出路径已存在，为保护数据拒绝覆盖: {}", output.display());
    }

    let parent = output_parent(output);
    let temp_path = unique_temp_path(parent)?;
    let mut guard = TempPathGuard::new(temp_path.clone());

    let bytes = match kind {
        ArchiveKind::TarZst => {
            fs::create_dir_all(&temp_path)
                .with_context(|| format!("创建临时解压目录失败: {}", temp_path.display()))?;
            decompress_tar_from_reader(reader, &temp_path, state)?;
            count_source_bytes_strict(&temp_path)?
        }
        ArchiveKind::Zst => decompress_file_from_reader(reader, &temp_path, state)?,
    };

    if output.exists() {
        bail!("输出路径已存在，为保护数据拒绝覆盖: {}", output.display());
    }

    fs::rename(&temp_path, output).with_context(|| {
        format!(
            "提交解压结果失败: {} -> {}",
            temp_path.display(),
            output.display()
        )
    })?;
    guard.disarm();

    sync_directory(parent);

    Ok(bytes)
}

/// Best-effort `fsync` of a directory so a completed rename survives a crash.
/// Directories cannot be opened as files on Windows, hence the `cfg`.
fn sync_directory(dir: &Path) {
    #[cfg(unix)]
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

fn output_parent(path: &Path) -> &Path {
    path.parent().unwrap_or_else(|| Path::new("."))
}

fn validate_compress_paths(
    source: &Path,
    output: &Path,
    kind: OutputKind,
    split_size: Option<u64>,
) -> Result<()> {
    let source_access = fs_access_path(source)
        .with_context(|| format!("无法准备源路径: {}", source.display()))?;
    let source = fs::canonicalize(&source_access)
        .with_context(|| format!("无法解析源路径: {}", source.display()))?;
    let output_parent = output_parent(output);
    let output_parent =
        fs::canonicalize(output_parent).unwrap_or_else(|_| output_parent.to_path_buf());
    let output_abs = output_parent.join(
        output
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("archive")),
    );

    if output_abs == source {
        bail!("输出路径不能覆盖源路径");
    }
    if source.is_dir() && output_abs.starts_with(&source) {
        bail!("输出路径不能位于待压缩目录内部");
    }
    ensure_output_available(output, kind, split_size)?;
    Ok(())
}

/// Refuse to start when anything we are about to create already exists.
///
/// `File::create` truncates in place, so a mistyped output path used to destroy
/// an unrelated file before a single byte was compressed. Decompression already
/// had this guard (`validate_decompress_paths`); compression did not.
fn ensure_output_available(
    output: &Path,
    kind: OutputKind,
    split_size: Option<u64>,
) -> Result<()> {
    let targets: Vec<PathBuf> = if split_enabled(split_size) {
        // Leftovers from a previous split run in the same spot: writing over
        // `.001` while `.002`+ survive produces a chain that looks complete and
        // decodes to garbage, so treat any surviving volume as a collision.
        let mut volumes = existing_volume_paths(output)?;
        if volumes.is_empty() {
            volumes.push(multi_volume_path(output, 1)?);
        }
        volumes
    } else if kind == OutputKind::SfxExe {
        vec![output.to_path_buf(), sfx::sidecar_path(output)]
    } else {
        vec![output.to_path_buf()]
    };

    for target in targets {
        if target.exists() {
            bail!("输出路径已存在，为保护数据拒绝覆盖: {}", target.display());
        }
    }
    Ok(())
}

fn validate_decompress_paths(archive: &Path, output: &Path, _meta: ArchiveMeta) -> Result<()> {
    if archive == output {
        bail!("解压输出不能覆盖归档文件");
    }
    if output.exists() {
        bail!("输出路径已存在，为保护数据拒绝覆盖: {}", output.display());
    }
    Ok(())
}

fn count_source_bytes_strict(path: &Path) -> Result<u64> {
    let access_path = fs_access_path(path)
        .with_context(|| format!("无法准备路径: {}", path.display()))?;
    let metadata = fs::metadata(&access_path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0_u64;
    for entry in WalkDir::new(&access_path) {
        let entry = entry.context("遍历目录失败")?;
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
}

fn detect_archive_meta(path: &Path) -> Result<ArchiveMeta> {
    let name = path
        .file_name()
        .map(|v| v.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // Check for multi-volume suffix .001, .002 ...
    let is_multi = name.len() > 4
        && name.as_bytes()[name.len() - 4] == b'.'
        && name.as_bytes()[name.len() - 3].is_ascii_digit()
        && name.as_bytes()[name.len() - 2].is_ascii_digit()
        && name.as_bytes()[name.len() - 1].is_ascii_digit();

    let mut meta_name = name.clone();
    if is_multi {
        meta_name = name[..name.len() - 4].to_string();
    }

    let encrypted = meta_name.ends_with(".enc");
    let base = if encrypted {
        meta_name.strip_suffix(".enc").unwrap_or(&meta_name)
    } else {
        &meta_name
    };

    let kind = if base.ends_with(".tar.zst") {
        ArchiveKind::TarZst
    } else if base.ends_with(".zst") {
        ArchiveKind::Zst
    } else {
        bail!("不支持的文件类型，仅支持 .zst/.tar.zst 及其 .enc 加密版本")
    };

    Ok(ArchiveMeta {
        kind,
        encrypted,
        is_multi_volume: is_multi,
    })
}

fn resolve_compress_output(
    source: &Path,
    output: Option<&str>,
    encrypted: bool,
    output_kind: OutputKind,
) -> Result<PathBuf> {
    let mut candidate = if let Some(path) = output {
        let provided = PathBuf::from(path.trim());
        if provided.exists() && provided.is_dir() {
            provided.join(default_compress_file_name(source, encrypted, output_kind)?)
        } else {
            provided
        }
    } else {
        let parent = source.parent().unwrap_or_else(|| Path::new("."));
        parent.join(default_compress_file_name(source, encrypted, output_kind)?)
    };

    match output_kind {
        OutputKind::Archive => {
            if encrypted {
                candidate = ensure_enc_suffix(candidate);
            }
        }
        OutputKind::SfxExe => {
            candidate = ensure_exe_suffix(candidate);
        }
    }

    Ok(candidate)
}

fn ensure_enc_suffix(path: PathBuf) -> PathBuf {
    let name_lower = path
        .file_name()
        .map(|v| v.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if name_lower.ends_with(".enc") {
        return path;
    }

    let file_name = path
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());

    path.with_file_name(format!("{file_name}.enc"))
}

fn ensure_exe_suffix(path: PathBuf) -> PathBuf {
    let name_lower = path
        .file_name()
        .map(|v| v.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if name_lower.ends_with(".exe") {
        return path;
    }

    let file_name = path
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());

    path.with_file_name(format!("{file_name}.exe"))
}

fn default_compress_file_name(
    source: &Path,
    encrypted: bool,
    output_kind: OutputKind,
) -> Result<String> {
    let source_name = source
        .file_name()
        .with_context(|| format!("无效路径: {}", source.display()))?
        .to_string_lossy();

    let mut name = match output_kind {
        OutputKind::Archive => {
            if source.is_dir() {
                format!("{source_name}.tar.zst")
            } else {
                format!("{source_name}.zst")
            }
        }
        OutputKind::SfxExe => format!("{source_name}.sfx.exe"),
    };

    if encrypted && output_kind == OutputKind::Archive {
        name.push_str(".enc");
    }

    Ok(name)
}

fn resolve_decompress_output(
    archive: &Path,
    meta: ArchiveMeta,
    output: Option<&str>,
) -> Result<PathBuf> {
    let default_name = default_decompress_name(archive, meta)?;

    match output {
        Some(path) => {
            let candidate = PathBuf::from(path.trim());
            match meta.kind {
                ArchiveKind::TarZst => Ok(candidate),
                ArchiveKind::Zst => {
                    if candidate.exists() && candidate.is_dir() {
                        Ok(candidate.join(default_name))
                    } else {
                        Ok(candidate)
                    }
                }
            }
        }
        None => {
            let parent = archive.parent().unwrap_or_else(|| Path::new("."));
            Ok(parent.join(default_name))
        }
    }
}

fn default_decompress_name(archive: &Path, meta: ArchiveMeta) -> Result<String> {
    let file_name = archive
        .file_name()
        .with_context(|| format!("无效路径: {}", archive.display()))?
        .to_string_lossy();

    let base = if meta.encrypted {
        file_name.trim_end_matches(".enc").to_string()
    } else {
        file_name.to_string()
    };

    match meta.kind {
        ArchiveKind::TarZst => {
            let stem = base.trim_end_matches(".tar.zst");
            Ok(format!("{stem}_extracted"))
        }
        ArchiveKind::Zst => {
            let stem = base.trim_end_matches(".zst");
            Ok(stem.to_string())
        }
    }
}

fn normalize_password(raw: Option<String>) -> Option<SecretString> {
    raw.and_then(|mut value| {
        let trimmed = value.trim().to_string();
        // Scrub the untrimmed copy we were handed; the normalized one scrubs
        // itself on drop.
        value.zeroize();
        if trimmed.is_empty() {
            None
        } else {
            Some(SecretString(trimmed))
        }
    })
}

/// Return a path suitable for direct filesystem access.
///
/// On Windows an ordinary path such as `D:\tree\NUL` is parsed through the
/// legacy DOS device namespace, so opening it targets the null device even when
/// a literal file named `NUL` exists on disk. The verbatim `\\?\` namespace
/// disables that name rewriting and also keeps long paths working. Other
/// platforms can use the original path unchanged.
#[cfg(windows)]
fn fs_access_path(path: &Path) -> io::Result<PathBuf> {
    use std::path::{Component, Prefix};

    let absolute = std::path::absolute(path)?;
    let mut components = absolute.components();
    let Some(Component::Prefix(prefix_component)) = components.next() else {
        return Ok(absolute);
    };

    let mut verbatim = match prefix_component.kind() {
        Prefix::Verbatim(_) | Prefix::VerbatimUNC(_, _) | Prefix::VerbatimDisk(_) => {
            return Ok(absolute);
        }
        Prefix::Disk(drive) => PathBuf::from(format!(r"\\?\{}:\", char::from(drive))),
        Prefix::UNC(server, share) => {
            let mut root = PathBuf::from(r"\\?\UNC\");
            root.push(server);
            root.push(share);
            root
        }
        Prefix::DeviceNS(_) => return Ok(absolute),
    };

    for component in components {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => verbatim.push(".."),
            Component::Normal(part) => verbatim.push(part),
        }
    }

    Ok(verbatim)
}

#[cfg(not(windows))]
fn fs_access_path(path: &Path) -> io::Result<PathBuf> {
    Ok(path.to_path_buf())
}

fn count_source_bytes(path: &Path) -> Result<u64> {
    let access_path = fs_access_path(path)
        .with_context(|| format!("无法准备源路径: {}", path.display()))?;
    let metadata = fs::metadata(&access_path)
        .with_context(|| format!("无法读取文件信息: {}", path.display()))?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0_u64;
    for entry in WalkDir::new(&access_path)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
        }
    }

    Ok(total)
}

fn throughput(bytes: u64, secs: f64) -> f64 {
    let safe_secs = secs.max(f64::EPSILON);
    (bytes as f64 / MIB) / safe_secs
}

fn ratio(output_bytes: u64, source_bytes: u64) -> f64 {
    if source_bytes == 0 {
        return 0.0;
    }
    output_bytes as f64 / source_bytes as f64 * 100.0
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            compress_archive,
            decompress_archive,
            extract_embedded_archive,
            benchmark_compression,
            abort_task,
            list_archive_content,
            get_embedded_archive_info,
            inspect_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn deterministic_bytes(size: usize) -> Vec<u8> {
        (0..size).map(|i| ((i * 131 + 17) % 251) as u8).collect()
    }

    fn pseudo_random_bytes(size: usize) -> Vec<u8> {
        let mut state = 0x1234_5678_9abc_def0_u64;
        let mut bytes = Vec::with_capacity(size);
        for _ in 0..size {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.push((state & 0xff) as u8);
        }
        bytes
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, bytes).expect("write file");
    }

    fn collect_file_map(root: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut map = BTreeMap::new();
        for entry in WalkDir::new(root)
            .min_depth(1)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let rel = entry
                .path()
                .strip_prefix(root)
                .expect("strip prefix")
                .to_string_lossy()
                .replace('\\', "/");

            map.insert(rel, fs::read(entry.path()).expect("read file"));
        }
        map
    }

    fn assert_dirs_equal(expected: &Path, actual: &Path) {
        assert_eq!(collect_file_map(expected), collect_file_map(actual));
    }

    fn archive_request(
        source: &Path,
        output: &Path,
        level: i32,
        include_root_dir: bool,
        password: Option<&str>,
        split_size_mib: Option<u64>,
    ) -> CompressRequest {
        CompressRequest {
            source_path: path_to_string(source),
            output_path: Some(path_to_string(output)),
            output_kind: Some(OutputKind::Archive),
            level: Some(level),
            include_root_dir: Some(include_root_dir),
            password: password.map(str::to_string),
            split_size_mib,
            enable_logging: Some(false),
            delete_source_after: Some(false),
            threads: Some(1),
        }
    }

    #[test]
    fn encrypted_roundtrip_file_sizes_and_types() {
        let sizes = [
            0_usize,
            1,
            31,
            4 * 1024,
            ENC_CHUNK_SIZE - 1,
            ENC_CHUNK_SIZE,
            ENC_CHUNK_SIZE + 1,
            ENC_CHUNK_SIZE * 3 + 123,
        ];

        for (idx, size) in sizes.into_iter().enumerate() {
            let temp = tempfile::tempdir().expect("temp dir");
            let source = temp.path().join(format!("data_{idx}.bin"));
            let archive = temp.path().join(format!("data_{idx}.zst.enc"));
            let output = temp.path().join(format!("out_{idx}.bin"));

            let payload = deterministic_bytes(size);
            write_file(&source, &payload);

            compress_archive_sync(
                archive_request(&source, &archive, 8, true, Some("Strong#Pass123"), None),
                None,
                None,
            )
            .expect("compress encrypted");

            decompress_archive_sync(
                DecompressRequest {
                    archive_path: path_to_string(&archive),
                    output_path: Some(path_to_string(&output)),
                    password: Some("Strong#Pass123".to_string()),
                },
                None,
                None,
            )
            .expect("decompress encrypted");

            let restored = fs::read(&output).expect("read output");
            assert_eq!(restored, payload, "size={size}");
        }
    }

    #[test]
    fn encrypted_roundtrip_directory_include_root_variants() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_dir = temp.path().join("project_src");
        fs::create_dir_all(&source_dir).expect("create source dir");

        write_file(&source_dir.join("empty.txt"), b"");
        write_file(&source_dir.join("plain.txt"), b"hello encrypted world");
        write_file(&source_dir.join("config.json"), br#"{"k":1,"v":"x"}"#);
        write_file(
            &source_dir.join("nested/bin.dat"),
            &deterministic_bytes(131_072),
        );
        write_file(
            &source_dir.join("nested/unicode/中文-emoji.txt"),
            "你好, encryption ✓".as_bytes(),
        );
        write_file(
            &source_dir.join("nested/huge/chunk.bin"),
            &deterministic_bytes(ENC_CHUNK_SIZE * 2 + 77),
        );

        for include_root in [true, false] {
            let archive = temp.path().join(format!("dir_{include_root}.tar.zst.enc"));
            let out_dir = temp.path().join(format!("out_{include_root}"));

            compress_archive_sync(
                archive_request(
                    &source_dir,
                    &archive,
                    6,
                    include_root,
                    Some("Dir#Secure987"),
                    None,
                ),
                None,
                None,
            )
            .expect("compress dir encrypted");

            decompress_archive_sync(
                DecompressRequest {
                    archive_path: path_to_string(&archive),
                    output_path: Some(path_to_string(&out_dir)),
                    password: Some("Dir#Secure987".to_string()),
                },
                None,
                None,
            )
            .expect("decompress dir encrypted");

            let actual_root = if include_root {
                out_dir.join("project_src")
            } else {
                out_dir.clone()
            };
            assert_dirs_equal(&source_dir, &actual_root);
        }
    }

    #[test]
    fn encrypted_archive_rejects_wrong_password() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("secret.bin");
        let archive = temp.path().join("secret.zst.enc");
        let output = temp.path().join("secret.out");

        write_file(&source, &deterministic_bytes(8192));

        compress_archive_sync(
            archive_request(&source, &archive, 5, true, Some("CorrectPassword"), None),
            None,
            None,
        )
        .expect("compress encrypted");

        let err = decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&archive),
                output_path: Some(path_to_string(&output)),
                password: Some("WrongPassword".to_string()),
            },
            None,
            None,
        )
        .expect_err("wrong password must fail");

        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn plain_roundtrip_still_works_after_encryption_feature() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("plain.bin");
        let archive = temp.path().join("plain.bin.zst");
        let output = temp.path().join("plain.out");

        let payload = deterministic_bytes(2 * 1024 * 1024 + 13);
        write_file(&source, &payload);

        compress_archive_sync(
            archive_request(&source, &archive, 9, true, None, None),
            None,
            None,
        )
        .expect("compress plain");

        decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&archive),
                output_path: Some(path_to_string(&output)),
                password: None,
            },
            None,
            None,
        )
        .expect("decompress plain");

        assert_eq!(fs::read(output).expect("read output"), payload);
    }

    #[test]
    fn literal_windows_reserved_name_roundtrips() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_dir = temp.path().join("reserved_source");
        fs::create_dir_all(&source_dir).expect("create source dir");

        let reserved_source = source_dir.join("NUL");
        let reserved_source_access = fs_access_path(&reserved_source).expect("source access path");
        fs::write(&reserved_source_access, b"literal NUL filename").expect("write NUL file");

        let archive = temp.path().join("reserved.tar.zst");
        compress_archive_sync(
            archive_request(&source_dir, &archive, 3, false, None, None),
            None,
            None,
        )
        .expect("compress reserved name");

        let output = temp.path().join("reserved_output");
        decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&archive),
                output_path: Some(path_to_string(&output)),
                password: None,
            },
            None,
            None,
        )
        .expect("decompress reserved name");

        let reserved_output = output.join("NUL");
        let reserved_output_access = fs_access_path(&reserved_output).expect("output access path");
        assert_eq!(
            fs::read(&reserved_output_access).expect("read restored NUL file"),
            b"literal NUL filename"
        );

        // On Windows, ordinary recursive cleanup cannot address a literal NUL
        // entry, so remove the special files through the verbatim namespace.
        fs::remove_file(&reserved_source_access).expect("remove source NUL file");
        fs::remove_file(&reserved_output_access).expect("remove restored NUL file");
    }

    #[test]
    fn split_archive_reports_first_volume_and_roundtrips() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("split.bin");
        let archive = temp.path().join("split.bin.zst");
        let output = temp.path().join("split.out");

        let payload = pseudo_random_bytes(3 * 1024 * 1024 + 113);
        write_file(&source, &payload);

        let report = compress_archive_sync(
            archive_request(&source, &archive, 1, true, None, Some(1)),
            None,
            None,
        )
        .expect("compress split archive");

        let first_volume = temp.path().join("split.bin.zst.001");
        let second_volume = temp.path().join("split.bin.zst.002");
        assert_eq!(report.output_path, path_to_string(&first_volume));
        assert!(first_volume.exists());
        assert!(second_volume.exists());
        assert_eq!(report.blake3_hash, None);

        let volumes = existing_volume_paths(&archive).expect("list split volumes");
        assert!(volumes.len() > 1);
        let expected_size: u64 = volumes
            .iter()
            .map(|path| fs::metadata(path).unwrap().len())
            .sum();
        assert_eq!(report.output_bytes, expected_size);

        decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&first_volume),
                output_path: Some(path_to_string(&output)),
                password: None,
            },
            None,
            None,
        )
        .expect("decompress split archive");

        assert_eq!(fs::read(output).expect("read output"), payload);
    }

    #[test]
    fn benchmark_compression_returns_recommendation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("blob.bin");
        write_file(&source_path, &deterministic_bytes(2 * 1024 * 1024));

        let report = benchmark_compression_sync(
            BenchmarkRequest {
                source_path: path_to_string(&source_path),
                min_level: Some(1),
                max_level: Some(4),
                iterations: Some(1),
                sample_size_mib: Some(16),
                threads: Some(1),
            },
            None,
        )
        .expect("benchmark");

        assert_eq!(report.results.len(), 4);
        assert!((1..=4).contains(&report.recommended_level));
        assert!(report.sample_bytes > 0);
    }

    // --- 加密层 ---

    #[test]
    fn legacy_zenc0001_archives_still_decrypt() {
        // Forward compatibility guard: `ZENC0002` writes its Argon2 parameters
        // into the header. A v1 archive has no such block, so the reader must
        // fall back to `LEGACY_KDF` and read the ciphertext at the right offset.
        let payload = pseudo_random_bytes(600 * 1024);
        let password = "legacy-pw";

        let mut encrypted = Vec::new();
        {
            let mut salt = [0_u8; ENC_SALT_LEN];
            let mut nonce_prefix = [0_u8; ENC_NONCE_PREFIX_LEN];
            OsRng.fill_bytes(&mut salt);
            OsRng.fill_bytes(&mut nonce_prefix);

            encrypted.extend_from_slice(ENC_MAGIC_V1);
            encrypted.extend_from_slice(&salt);
            encrypted.extend_from_slice(&nonce_prefix);

            let key = derive_encryption_key(password, &salt, LEGACY_KDF).expect("derive");
            let cipher = key.cipher();
            for (index, chunk) in payload.chunks(ENC_CHUNK_SIZE).enumerate() {
                let nonce = make_nonce(nonce_prefix, index as u64);
                let mut buffer = chunk.to_vec();
                cipher
                    .encrypt_in_place(XNonce::from_slice(&nonce), &[], &mut buffer)
                    .expect("encrypt");
                encrypted.extend_from_slice(&(buffer.len() as u32).to_be_bytes());
                encrypted.extend_from_slice(&buffer);
            }
            encrypted.extend_from_slice(&0_u32.to_be_bytes());
        }

        let mut reader =
            EncryptedReader::new(io::Cursor::new(encrypted), password).expect("open v1 reader");
        let mut decrypted = Vec::new();
        reader.read_to_end(&mut decrypted).expect("read v1 payload");
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn zenc0002_header_carries_kdf_params() {
        let mut sink = Vec::new();
        {
            let mut writer = EncryptedWriter::new(&mut sink, "pw").expect("writer");
            writer.write_all(b"hello").expect("write");
            writer.finish().expect("finish");
        }

        assert_eq!(&sink[..8], ENC_MAGIC_V2);
        let kdf_offset = 8 + ENC_SALT_LEN + ENC_NONCE_PREFIX_LEN;
        let mut raw = [0_u8; 12];
        raw.copy_from_slice(&sink[kdf_offset..kdf_offset + 12]);
        assert_eq!(KdfParams::from_bytes(raw).expect("parse kdf"), CURRENT_KDF);
    }

    #[test]
    fn hostile_kdf_params_are_rejected() {
        // Storing the KDF parameters in the header means a crafted archive could
        // ask us to allocate an absurd amount of memory during `derive`. The
        // bounds check must reject it before Argon2 ever sees it.
        let huge = KdfParams {
            m_cost_kib: u32::MAX,
            t_cost: 2,
            parallelism: 1,
        };
        assert!(KdfParams::from_bytes(huge.to_bytes()).is_err());

        let zeroed = KdfParams {
            m_cost_kib: 0,
            t_cost: 0,
            parallelism: 0,
        };
        assert!(KdfParams::from_bytes(zeroed.to_bytes()).is_err());
    }

    #[test]
    fn absurd_chunk_length_is_rejected_without_allocating() {
        // A 4-byte length prefix is fully attacker-controlled. Before the bound
        // check, `resize(chunk_len, 0)` on a forged `0xFFFFFFFF` tried to reserve
        // 4 GiB and aborted the process.
        let mut sink = Vec::new();
        {
            let mut writer = EncryptedWriter::new(&mut sink, "pw").expect("writer");
            writer.write_all(b"payload").expect("write");
            writer.finish().expect("finish");
        }

        let header_len = 8 + ENC_SALT_LEN + ENC_NONCE_PREFIX_LEN + 12;
        let mut forged = sink[..header_len].to_vec();
        forged.extend_from_slice(&u32::MAX.to_be_bytes());
        forged.extend_from_slice(&[0_u8; 64]);

        let mut reader = EncryptedReader::new(io::Cursor::new(forged), "pw").expect("reader");
        let mut out = Vec::new();
        let err = reader.read_to_end(&mut out).expect_err("must reject");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("加密分块长度非法"));
    }

    // --- 输出保护 ---

    #[test]
    fn compress_refuses_to_overwrite_existing_output() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("input.bin");
        write_file(&source_path, &deterministic_bytes(4096));

        let output = temp.path().join("precious.tar.zst");
        write_file(&output, b"do not destroy me");

        let err = compress_archive_sync(
            archive_request(&source_path, &output, 3, true, None, None),
            None,
            None,
        )
        .expect_err("must refuse");

        assert!(err.to_string().contains("输出路径已存在"));
        assert_eq!(fs::read(&output).expect("read output"), b"do not destroy me");
    }

    #[test]
    fn split_compress_refuses_to_overwrite_existing_volume() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("input.bin");
        write_file(&source_path, &pseudo_random_bytes(512 * 1024));

        let output = temp.path().join("bundle.tar.zst");
        let stale_second = temp.path().join("bundle.tar.zst.002");
        write_file(&stale_second, b"stale volume from an unrelated archive");

        let err = compress_archive_sync(
            archive_request(&source_path, &output, 3, true, None, Some(1)),
            None,
            None,
        )
        .expect_err("must refuse");

        assert!(err.to_string().contains("输出路径已存在"));
        assert!(stale_second.exists());
    }

    // --- 分卷链 ---

    #[test]
    fn volume_base_path_strips_any_index() {
        assert_eq!(
            volume_base_path(Path::new("/tmp/a.tar.zst.003")),
            PathBuf::from("/tmp/a.tar.zst")
        );
        assert_eq!(
            volume_base_path(Path::new("/tmp/a.tar.zst.001")),
            PathBuf::from("/tmp/a.tar.zst")
        );
        // Not a volume suffix: must be left alone.
        assert_eq!(
            volume_base_path(Path::new("/tmp/a.tar.zst")),
            PathBuf::from("/tmp/a.tar.zst")
        );
    }

    #[test]
    fn decompress_accepts_any_volume_and_detects_gaps() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("payload.bin");
        let payload = pseudo_random_bytes(3 * 1024 * 1024);
        write_file(&source_path, &payload);

        let archive = temp.path().join("payload.bin.zst");
        compress_archive_sync(
            archive_request(&source_path, &archive, 3, true, None, Some(1)),
            None,
            None,
        )
        .expect("compress split");

        let volumes = existing_volume_paths(&archive).expect("volumes");
        assert!(
            volumes.len() >= 3,
            "expected several volumes, got {}",
            volumes.len()
        );

        // Picking a middle volume used to fail with "未找到分卷归档首卷".
        let output = temp.path().join("restored.bin");
        decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&volumes[2]),
                output_path: Some(path_to_string(&output)),
                password: None,
            },
            None,
            None,
        )
        .expect("decompress from middle volume");
        assert_eq!(fs::read(&output).expect("read output"), payload);

        // A hole in the chain must be named, not silently truncated.
        fs::remove_file(&volumes[1]).expect("remove volume 2");
        let err = decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&volumes[0]),
                output_path: Some(path_to_string(&temp.path().join("restored2.bin"))),
                password: None,
            },
            None,
            None,
        )
        .expect_err("gap must be detected");
        assert!(
            err.to_string().contains("缺少第 2 卷"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn split_archive_hash_covers_every_volume() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("payload.bin");
        write_file(&source_path, &pseudo_random_bytes(3 * 1024 * 1024));

        let archive = temp.path().join("payload.bin.zst");
        compress_archive_sync(
            archive_request(&source_path, &archive, 3, true, None, Some(1)),
            None,
            None,
        )
        .expect("compress split");

        let volumes = existing_volume_paths(&archive).expect("volumes");
        assert!(volumes.len() >= 2);

        let meta = detect_archive_meta(&volumes[0]).expect("meta");
        let whole = calculate_archive_hash(&volumes[0], meta).expect("hash");
        let first_only = calculate_file_hash(&volumes[0]).expect("hash first");
        assert_ne!(
            whole, first_only,
            "hash must cover the whole chain, not just .001"
        );

        // Reproduce it independently by concatenating the volumes.
        let mut expected = blake3::Hasher::new();
        for volume in &volumes {
            expected.update(&fs::read(volume).expect("read volume"));
        }
        assert_eq!(whole, expected.finalize().to_hex().to_string());
    }

    #[test]
    fn multi_volume_writer_rotates_without_degrading_to_one_byte_writes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let base = temp.path().join("chunked.bin");
        let payload = deterministic_bytes(5 * 1024 * 1024);

        {
            let mut writer = MultiVolumeWriter::new(base.clone(), 1);
            writer.write_all(&payload).expect("write");
            writer.flush().expect("flush");
        }

        let volumes = existing_volume_paths(&base).expect("volumes");
        assert_eq!(volumes.len(), 5);
        for volume in &volumes {
            assert_eq!(fs::metadata(volume).expect("meta").len(), 1024 * 1024);
        }

        let mut rejoined = Vec::new();
        for volume in &volumes {
            rejoined.extend_from_slice(&fs::read(volume).expect("read"));
        }
        assert_eq!(rejoined, payload);
    }

    #[test]
    fn multi_volume_writer_survives_absurd_volume_size() {
        // `volume_limit_mib * 1024 * 1024` overflowed u64 and wrapped to a tiny
        // limit, shredding the archive. Now it saturates to "one huge volume".
        let temp = tempfile::tempdir().expect("temp dir");
        let base = temp.path().join("huge.bin");
        let payload = deterministic_bytes(64 * 1024);

        {
            let mut writer = MultiVolumeWriter::new(base.clone(), u64::MAX);
            writer.write_all(&payload).expect("write");
            writer.flush().expect("flush");
        }

        let volumes = existing_volume_paths(&base).expect("volumes");
        assert_eq!(volumes.len(), 1);
        assert_eq!(fs::read(&volumes[0]).expect("read"), payload);
    }

    // --- 事务性解压 ---

    #[test]
    fn failed_extraction_leaves_no_staging_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("tree");
        write_file(&source.join("a.bin"), &pseudo_random_bytes(512 * 1024));
        write_file(&source.join("nested/b.bin"), &pseudo_random_bytes(512 * 1024));

        let archive = temp.path().join("tree.tar.zst");
        compress_archive_sync(
            archive_request(&source, &archive, 3, true, None, None),
            None,
            None,
        )
        .expect("compress");

        // Truncate mid-stream so tar extraction fails partway through.
        let raw = fs::read(&archive).expect("read archive");
        let broken = temp.path().join("broken.tar.zst");
        fs::write(&broken, &raw[..raw.len() / 2]).expect("write truncated");

        let output_dir = temp.path().join("dest");
        fs::create_dir_all(&output_dir).expect("create dest");
        let output = output_dir.join("tree_extracted");

        decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&broken),
                output_path: Some(path_to_string(&output)),
                password: None,
            },
            None,
            None,
        )
        .expect_err("truncated archive must fail");

        assert!(!output.exists(), "output must not be created on failure");
        let leftovers: Vec<_> = fs::read_dir(&output_dir)
            .expect("read dest")
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            leftovers.is_empty(),
            "staging path leaked into the output directory: {leftovers:?}"
        );
    }

    #[test]
    fn concurrent_extractions_do_not_share_a_staging_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let parent = temp.path();
        let first = unique_temp_path(parent).expect("first");
        let second = unique_temp_path(parent).expect("second");
        assert_ne!(
            first, second,
            "two extractions in one directory must not collide"
        );
    }

    // --- 预览 ---

    #[test]
    fn preview_reports_real_size_and_rejects_wrong_password() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("blob.bin");
        let payload = pseudo_random_bytes(700 * 1024);
        write_file(&source_path, &payload);

        let archive = temp.path().join("blob.bin.zst.enc");
        compress_archive_sync(
            archive_request(&source_path, &archive, 3, true, Some("pw123"), None),
            None,
            None,
        )
        .expect("compress");

        let report = list_archive_content_sync(
            DecompressRequest {
                archive_path: path_to_string(&archive),
                output_path: None,
                password: Some("pw123".to_string()),
            },
            None,
            None,
        )
        .expect("preview");

        assert_eq!(report.total_files, 1);
        // Used to be hardcoded to 0 for single-file archives.
        assert_eq!(report.uncompressed_size, payload.len() as u64);
        assert_eq!(report.entries[0].path, "blob.bin");

        // Used to succeed with the wrong password because the `Zst` arm never
        // looked at it.
        let err = list_archive_content_sync(
            DecompressRequest {
                archive_path: path_to_string(&archive),
                output_path: None,
                password: Some("wrong".to_string()),
            },
            None,
            None,
        )
        .expect_err("wrong password must fail");
        // `to_string()` shows only the outermost context; `{:#}` walks the chain.
        let chain = format!("{err:#}");
        assert!(
            chain.contains("解密失败") || chain.contains("密码"),
            "unexpected error: {chain}"
        );
    }

    #[test]
    fn preview_hash_matches_full_volume_chain() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("payload.bin");
        write_file(&source_path, &pseudo_random_bytes(3 * 1024 * 1024));

        let archive = temp.path().join("payload.bin.zst");
        compress_archive_sync(
            archive_request(&source_path, &archive, 3, true, None, Some(1)),
            None,
            None,
        )
        .expect("compress split");

        let volumes = existing_volume_paths(&archive).expect("volumes");
        let report = list_archive_content_sync(
            DecompressRequest {
                archive_path: path_to_string(&volumes[0]),
                output_path: None,
                password: None,
            },
            None,
            None,
        )
        .expect("preview split");

        let meta = detect_archive_meta(&volumes[0]).expect("meta");
        assert_eq!(
            report.hash,
            calculate_archive_hash(&volumes[0], meta).expect("hash")
        );
    }

    // --- 符号链接 ---

    #[cfg(unix)]
    #[test]
    fn directory_roundtrip_preserves_symlinks() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("tree");
        write_file(&source.join("real.txt"), b"real content");
        std::os::unix::fs::symlink("real.txt", source.join("link.txt")).expect("symlink");

        let archive = temp.path().join("tree.tar.zst");
        compress_archive_sync(
            archive_request(&source, &archive, 3, false, None, None),
            None,
            None,
        )
        .expect("compress");

        let output = temp.path().join("restored");
        decompress_archive_sync(
            DecompressRequest {
                archive_path: path_to_string(&archive),
                output_path: Some(path_to_string(&output)),
                password: None,
            },
            None,
            None,
        )
        .expect("decompress");

        let restored_link = output.join("link.txt");
        let link_meta = fs::symlink_metadata(&restored_link).expect("symlink metadata");
        assert!(
            link_meta.file_type().is_symlink(),
            "symlink was dropped or materialised as a regular file"
        );
        assert_eq!(
            fs::read_link(&restored_link).expect("read_link"),
            PathBuf::from("real.txt")
        );
    }

    // --- 基准测试 ---

    #[test]
    fn benchmark_sample_fills_requested_size() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_path = temp.path().join("blob.bin");
        write_file(&source_path, &deterministic_bytes(9 * 1024 * 1024));

        // 3 MiB cap over a 9 MiB file: sampled from head/middle/tail, so we
        // should get very close to the cap rather than a short read's worth.
        let sample = load_benchmark_sample(&source_path, 3 * 1024 * 1024).expect("sample");
        assert_eq!(sample.len(), 3 * (1024 * 1024));
    }

    #[test]
    fn benchmark_honours_abort_between_chunks() {
        let state = AppState::new();
        state.request_abort();
        let err = compress_to_count(&pseudo_random_bytes(8 * 1024 * 1024), 3, 1, Some(&state))
            .expect_err("must abort");
        assert!(err.to_string().contains("终止"));
    }
}

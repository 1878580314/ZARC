import { invoke } from '@tauri-apps/api/core';

export type OutputKind = 'archive' | 'sfxExe';
export type ProgressKind = 'compress' | 'decompress' | 'benchmark';
export type ViewId = 'compress' | 'decompress' | 'benchmark';

export interface OperationReport {
  operation: string;
  sourcePath: string;
  outputPath: string;
  sourceBytes: number;
  outputBytes: number;
  durationMs: number;
  throughputMiBs: number;
  compressionRatio: number | null;
  blake3Hash: string | null;
  sidecarPath?: string | null;
}

export interface ArchiveEntry {
  path: string;
  size: number;
  isDir: boolean;
}

export interface ArchiveContentReport {
  entries: ArchiveEntry[];
  totalFiles: number;
  uncompressedSize: number;
  hash: string;
}

export interface EmbeddedArchiveInfo {
  hostPath: string;
  payloadBytes: number;
  defaultExtractName: string;
  encrypted: boolean;
  archiveKind: string;
}

export interface ProgressPayload {
  operation: ProgressKind;
  processedBytes: number;
  totalBytes: number;
  percent: number;
  throughputMiBs: number;
  etaSeconds: number | null;
  done: boolean;
  error: string | null;
}

export interface CompressionLevelReport {
  level: number;
  meanMs: number;
  meanThroughputMiBs: number;
  compressedBytes: number;
  ratioPercent: number;
  score: number;
}

export interface BenchmarkReport {
  sourcePath: string;
  sampleBytes: number;
  minLevel: number;
  maxLevel: number;
  iterations: number;
  threads: number;
  recommendedLevel: number;
  results: CompressionLevelReport[];
  note: string;
}

export interface CompressRequest {
  sourcePath: string;
  outputPath: string | null;
  outputKind: OutputKind;
  level: number;
  includeRootDir: boolean;
  password: string | null;
  splitSizeMib: number | null;
  enableLogging: boolean;
  deleteSourceAfter: boolean;
  /** zstd 工作线程数；null 表示交由后端使用全部核心。 */
  threads: number | null;
}

export interface DecompressRequest {
  archivePath: string;
  outputPath?: string | null;
  password?: string | null;
}

export interface EmbeddedDecompressRequest {
  outputPath: string | null;
  password: string | null;
}

export interface BenchmarkRequest {
  sourcePath: string;
  minLevel?: number;
  maxLevel?: number;
  iterations?: number;
  /** 必须与 Rust 端 `sample_size_mib` 的 camelCase 形式完全一致。 */
  sampleSizeMib?: number;
  threads?: number | null;
}

export interface PathInfo {
  path: string;
  exists: boolean;
  isDir: boolean;
  sizeBytes: number;
  fileCount: number;
  /** 目录条目数超过后端上限时为 true，此时 sizeBytes/fileCount 只是下界。 */
  truncated: boolean;
}

export const api = {
  compress: (r: CompressRequest) =>
    invoke<OperationReport>('compress_archive', { request: r }),
  decompress: (r: DecompressRequest) =>
    invoke<OperationReport>('decompress_archive', { request: r }),
  extractEmbedded: (r: EmbeddedDecompressRequest) =>
    invoke<OperationReport>('extract_embedded_archive', { request: r }),
  listContent: (r: DecompressRequest) =>
    invoke<ArchiveContentReport>('list_archive_content', { request: r }),
  benchmark: (r: BenchmarkRequest) =>
    invoke<BenchmarkReport>('benchmark_compression', { request: r }),
  getEmbeddedInfo: () =>
    invoke<EmbeddedArchiveInfo | null>('get_embedded_archive_info'),
  inspectPath: (path: string) => invoke<PathInfo>('inspect_path', { path }),
  abort: () => invoke('abort_task')
};

import type { OperationReport } from './api';
import { api } from './api';
import { t, numberLocale } from './i18n/index.svelte';
import { translateBackendText } from './i18n/backend';

export type PathKind = 'file' | 'folder';

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return '-';
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ['KiB', 'MiB', 'GiB', 'TiB'];
  let value = bytes;
  let unitIndex = -1;
  do {
    value /= 1024;
    unitIndex += 1;
  } while (value >= 1024 && unitIndex < units.length - 1);

  return `${value.toFixed(2)} ${units[unitIndex]}`;
}

export function formatSeconds(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return '-';
  }
  if (seconds < 60) {
    return `${seconds.toFixed(1)} ${t('time.sec')}`;
  }
  const mins = Math.floor(seconds / 60);
  const secs = Math.round(seconds % 60);
  if (mins < 60) {
    return `${mins} ${t('time.min')} ${secs} ${t('time.sec')}`;
  }
  return `${Math.floor(mins / 60)} ${t('time.hour')} ${mins % 60} ${t('time.min')}`;
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) {
    return '-';
  }
  return ms < 1000 ? `${ms.toFixed(0)} ms` : formatSeconds(ms / 1000);
}

export function formatCount(value: number): string {
  return value.toLocaleString(numberLocale());
}

export interface ReportField {
  label: string;
  value: string;
  /** Long hashes / long paths need a monospace font and wrapping; regular fields do not. */
  mono?: boolean;
}

/**
 * Splits the backend report into structured fields.
 *
 * The old implementation returned one long `\n`-joined string dumped into a `<pre>`,
 * which allowed neither proper layout nor copying paths individually.
 */
export function operationFields(report: OperationReport): ReportField[] {
  const fields: ReportField[] = [
    { label: t('field.sourcePath'), value: report.sourcePath, mono: true },
    { label: t('field.outputPath'), value: report.outputPath, mono: true },
    { label: t('field.sourceSize'), value: formatBytes(report.sourceBytes) },
    { label: t('field.outputSize'), value: formatBytes(report.outputBytes) },
    { label: t('field.throughput'), value: `${report.throughputMiBs.toFixed(2)} MiB/s` }
  ];
  if (report.compressionRatio !== null) {
    fields.push({ label: t('field.ratio'), value: `${report.compressionRatio.toFixed(2)}%` });
  }
  if (report.sidecarPath) {
    fields.push({ label: t('field.dataFile'), value: report.sidecarPath, mono: true });
  }
  if (report.blake3Hash) {
    fields.push({ label: 'BLAKE3', value: report.blake3Hash, mono: true });
  }
  return fields;
}

/** The three big numbers at the top of the result card. */
export function operationHighlights(report: OperationReport): ReportField[] {
  const saved = report.sourceBytes - report.outputBytes;
  return [
    { label: t('field.outputSize'), value: formatBytes(report.outputBytes) },
    {
      label: report.compressionRatio === null ? t('field.sizeChange') : t('field.ratio'),
      value:
        report.compressionRatio === null
          ? `${saved >= 0 ? '−' : '+'}${formatBytes(Math.abs(saved))}`
          : `${report.compressionRatio.toFixed(1)}%`
    },
    { label: t('field.elapsed'), value: formatDuration(report.durationMs) }
  ];
}

export function normalizeError(error: unknown): string {
  let message: string;
  if (typeof error === 'string') {
    message = error;
  } else if (error instanceof Error) {
    message = error.message;
  } else if (error && typeof error === 'object') {
    message = String(error);
  } else {
    message = t('error.unknown');
  }
  return translateBackendText(message);
}

/** File-type icon key, resolved to an inline SVG by `Icon.svelte`. */
export function getFileIcon(filename: string): string {
  const ext = filename.split('.').pop()?.toLowerCase();
  switch (ext) {
    case 'zst':
    case 'enc':
    case 'zip':
    case 'rar':
    case '7z':
    case 'gz':
    case 'tar':
      return 'archive';
    case 'exe':
    case 'sh':
    case 'app':
    case 'msi':
    case 'bat':
      return 'app';
    case 'jpg':
    case 'jpeg':
    case 'png':
    case 'webp':
    case 'gif':
    case 'svg':
    case 'bmp':
      return 'image';
    case 'mp4':
    case 'mkv':
    case 'mov':
    case 'avi':
    case 'webm':
      return 'video';
    case 'mp3':
    case 'wav':
    case 'flac':
    case 'ogg':
    case 'm4a':
      return 'audio';
    case 'pdf':
    case 'txt':
    case 'md':
    case 'log':
      return 'text';
    case 'ts':
    case 'js':
    case 'rs':
    case 'py':
    case 'go':
    case 'json':
    case 'toml':
    case 'yaml':
    case 'yml':
    case 'html':
    case 'css':
      return 'code';
    default:
      return 'file';
  }
}

export function pathBaseName(path: string): string {
  const parts = path.split(/[\\/]/).filter((p) => p.length > 0);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

/** ZARC volume suffixes look like `.001`: at least three characters, all digits (aligned with the backend `is_volume_suffix`). */
function isVolumeSuffix(suffix: string): boolean {
  return suffix.length >= 3 && /^\d+$/.test(suffix);
}

export function isArchivePath(path: string): boolean {
  const base = pathBaseName(path).toLowerCase();
  const suffix = base.includes('.') ? base.slice(base.lastIndexOf('.') + 1) : '';
  return suffix === 'zst' || suffix === 'enc' || suffix === 'exe' || isVolumeSuffix(suffix);
}

/**
 * Asks the file system what this path actually is.
 *
 * The old implementation guessed with `basename.includes('.')`, so `release.v2/` was
 * classified as a file and `Makefile` as a folder, passing the wrong `includeRootDir`
 * semantics to the backend. Only on IPC failure do we fall back to that heuristic.
 */
export async function pathKindLabel(path: string): Promise<PathKind> {
  try {
    const info = await api.inspectPath(path);
    if (info.exists) {
      return info.isDir ? 'folder' : 'file';
    }
  } catch {
    // Fall through to the heuristic below.
  }
  return pathBaseName(path).includes('.') ? 'file' : 'folder';
}

export function toInt(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

export function emptyToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export interface PasswordStrength {
  score: 0 | 1 | 2 | 3 | 4;
  label: string;
  hint: string;
}

/**
 * Coarse-grained password strength estimate.
 *
 * A local hint only - it never blocks submission. The backend derives the key with
 * Argon2id, so the cost of a weak password is the user's to bear, but they should
 * at least be able to see it coming.
 */
export function passwordStrength(password: string): PasswordStrength {
  if (password.length === 0) {
    return { score: 0, label: t('pw.notSet'), hint: t('pw.hint.notSet') };
  }

  const classes =
    Number(/[a-z]/.test(password)) +
    Number(/[A-Z]/.test(password)) +
    Number(/\d/.test(password)) +
    Number(/[^\w\s]/.test(password));

  let score = 0;
  if (password.length >= 8) score += 1;
  if (password.length >= 14) score += 1;
  if (classes >= 3) score += 1;
  if (classes >= 4 && password.length >= 12) score += 1;

  const table: PasswordStrength[] = [
    { score: 0, label: t('pw.veryWeak'), hint: t('pw.hint.veryWeak') },
    { score: 1, label: t('pw.weak'), hint: t('pw.hint.weak') },
    { score: 2, label: t('pw.fair'), hint: t('pw.hint.fair') },
    { score: 3, label: t('pw.strong'), hint: t('pw.hint.strong') },
    { score: 4, label: t('pw.veryStrong'), hint: t('pw.hint.veryStrong') }
  ];
  return table[score];
}

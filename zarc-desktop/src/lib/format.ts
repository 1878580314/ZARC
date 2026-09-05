import type { OperationReport } from './api';
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
  /** 长哈希/长路径需要等宽字体与换行；普通字段则不需要。 / Long hashes / long paths need a monospace font and wrapping; regular fields do not. */
  mono?: boolean;
}

/**
 * 将后端报告拆分为结构化字段。
 *
 * 旧实现返回一个以 `\n` 拼接的长字符串并直接塞进 `<pre>`，
 * 既无法正常排版，也无法单独复制路径。
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

/** 结果卡片顶部的三个大数字。 / The three big numbers at the top of the result card. */
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

/** 文件类型图标键，由 `Icon.svelte` 解析为内联 SVG。 / File-type icon key, resolved to an inline SVG by `Icon.svelte`. */
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

/** SFX 宿主旁的 sidecar 载荷文件名（与后端 `SIDECAR_SUFFIX` 对齐）。
 *  Expected sidecar payload name next to an SFX host (aligned with backend `SIDECAR_SUFFIX`). */
export function sidecarName(hostPath: string): string {
  return `${pathBaseName(hostPath)}.payload`;
}

/** ZARC 分卷后缀形如 `.001`：至少三个字符且全为数字（与后端 `is_volume_suffix` 对齐）。 / ZARC volume suffixes look like `.001`: at least three characters, all digits (aligned with the backend `is_volume_suffix`). */
function isVolumeSuffix(suffix: string): boolean {
  return suffix.length >= 3 && /^\d+$/.test(suffix);
}

export function isArchivePath(path: string): boolean {
  const base = pathBaseName(path).toLowerCase();
  const suffix = base.includes('.') ? base.slice(base.lastIndexOf('.') + 1) : '';
  return suffix === 'zst' || suffix === 'enc' || suffix === 'exe' || isVolumeSuffix(suffix);
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
 * 粗粒度的密码强度估算。
 *
 * 仅本地提示——绝不阻止提交。后端用 Argon2id 派生密钥，
 * 弱密码的代价由用户承担，但至少应让用户能提前看到。
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

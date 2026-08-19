import type { OperationReport } from './api';
import { api } from './api';

export type PathKind = '文件' | '目录';

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
    return `${seconds.toFixed(1)} 秒`;
  }
  const mins = Math.floor(seconds / 60);
  const secs = Math.round(seconds % 60);
  if (mins < 60) {
    return `${mins} 分 ${secs} 秒`;
  }
  return `${Math.floor(mins / 60)} 小时 ${mins % 60} 分`;
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) {
    return '-';
  }
  return ms < 1000 ? `${ms.toFixed(0)} ms` : formatSeconds(ms / 1000);
}

export function formatCount(value: number): string {
  return value.toLocaleString('zh-CN');
}

export interface ReportField {
  label: string;
  value: string;
  /** 长哈希 / 长路径需要等宽字体与换行，普通字段不需要。 */
  mono?: boolean;
}

/**
 * 把后端报告拆成结构化字段。
 *
 * 旧实现返回一整段 `\n` 拼接的字符串塞进 `<pre>`，既无法排版也无法单独复制路径。
 */
export function operationFields(report: OperationReport): ReportField[] {
  const fields: ReportField[] = [
    { label: '源路径', value: report.sourcePath, mono: true },
    { label: '输出路径', value: report.outputPath, mono: true },
    { label: '源大小', value: formatBytes(report.sourceBytes) },
    { label: '结果大小', value: formatBytes(report.outputBytes) },
    { label: '吞吐', value: `${report.throughputMiBs.toFixed(2)} MiB/s` }
  ];
  if (report.compressionRatio !== null) {
    fields.push({ label: '压缩率', value: `${report.compressionRatio.toFixed(2)}%` });
  }
  if (report.sidecarPath) {
    fields.push({ label: '数据文件', value: report.sidecarPath, mono: true });
  }
  if (report.blake3Hash) {
    fields.push({ label: 'BLAKE3', value: report.blake3Hash, mono: true });
  }
  return fields;
}

/** 结果卡片顶部的三个大数字。 */
export function operationHighlights(report: OperationReport): ReportField[] {
  const saved = report.sourceBytes - report.outputBytes;
  return [
    { label: '结果大小', value: formatBytes(report.outputBytes) },
    {
      label: report.compressionRatio === null ? '体积变化' : '压缩率',
      value:
        report.compressionRatio === null
          ? `${saved >= 0 ? '−' : '+'}${formatBytes(Math.abs(saved))}`
          : `${report.compressionRatio.toFixed(1)}%`
    },
    { label: '耗时', value: formatDuration(report.durationMs) }
  ];
}

export function normalizeError(error: unknown): string {
  if (typeof error === 'string') {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (error && typeof error === 'object') {
    return String(error);
  }
  return '发生未知错误。';
}

/** 文件类型图标键，由 `Icon.svelte` 解析成内联 SVG。 */
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

/** ZARC 分卷后缀形如 `.001`：至少三位且全为数字（与后端 `is_volume_suffix` 对齐）。 */
function isVolumeSuffix(suffix: string): boolean {
  return suffix.length >= 3 && /^\d+$/.test(suffix);
}

export function isArchivePath(path: string): boolean {
  const base = pathBaseName(path).toLowerCase();
  const suffix = base.includes('.') ? base.slice(base.lastIndexOf('.') + 1) : '';
  return suffix === 'zst' || suffix === 'enc' || suffix === 'exe' || isVolumeSuffix(suffix);
}

/**
 * 询问文件系统这条路径到底是什么。
 *
 * 旧实现用 `basename.includes('.')` 猜，于是 `release.v2/` 被判成文件、
 * `Makefile` 被判成目录，进而把错误的 `includeRootDir` 语义传给后端。
 * 只有 IPC 失败时才退回那个启发式。
 */
export async function pathKindLabel(path: string): Promise<PathKind> {
  try {
    const info = await api.inspectPath(path);
    if (info.exists) {
      return info.isDir ? '目录' : '文件';
    }
  } catch {
    // 落到下面的启发式。
  }
  return pathBaseName(path).includes('.') ? '文件' : '目录';
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
 * 粗粒度密码强度评估。
 *
 * 只做本地提示，不阻断提交——后端用 Argon2id 派生密钥，弱密码的代价由用户
 * 自行承担，但至少要让他们看见这个代价。
 */
export function passwordStrength(password: string): PasswordStrength {
  if (password.length === 0) {
    return { score: 0, label: '未设置', hint: '加密归档必须设置密码。' };
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
    { score: 0, label: '很弱', hint: '至少 8 位，并混合大小写、数字与符号。' },
    { score: 1, label: '较弱', hint: '再加长一些，并混合多种字符类型。' },
    { score: 2, label: '中等', hint: '建议达到 14 位以上。' },
    { score: 3, label: '较强', hint: '已足够日常使用。' },
    { score: 4, label: '很强', hint: '密码遗失后归档无法恢复，请妥善保存。' }
  ];
  return table[score];
}

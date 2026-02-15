import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import './style.css';

type ProgressKind = 'compress' | 'decompress';

interface OperationReport {
  operation: string;
  sourcePath: string;
  outputPath: string;
  sourceBytes: number;
  outputBytes: number;
  durationMs: number;
  throughputMiBs: number;
  compressionRatio: number | null;
  blake3Hash: string | null;
}

interface ArchiveEntry {
  path: string;
  size: number;
  isDir: boolean;
}

interface ArchiveContentReport {
  entries: ArchiveEntry[];
  totalFiles: number;
  uncompressedSize: number;
  hash: string;
}

interface ProgressPayload {
  operation: ProgressKind;
  processedBytes: number;
  totalBytes: number;
  percent: number;
  throughputMiBs: number;
  etaSeconds: number | null;
  done: boolean;
  error: string | null;
}

interface CompressionLevelReport {
  level: number;
  meanMs: number;
  meanThroughputMiBs: number;
  compressedBytes: number;
  ratioPercent: number;
  score: number;
}

interface CompressionBenchmarkReport {
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

const compressSource = byId<HTMLInputElement>('compressSource');
const compressOutput = byId<HTMLInputElement>('compressOutput');
const compressLevel = byId<HTMLInputElement>('compressLevel');
const compressLevelLabel = byId<HTMLSpanElement>('compressLevelLabel');
const compressKindTag = byId<HTMLSpanElement>('compressKindTag');
const includeRootDir = byId<HTMLInputElement>('includeRootDir');
const compressEncrypt = byId<HTMLInputElement>('compressEncrypt');
const compressPassword = byId<HTMLInputElement>('compressPassword');
const compressResult = byId<HTMLElement>('compressResult');

const decompressSource = byId<HTMLInputElement>('decompressSource');
const decompressOutput = byId<HTMLInputElement>('decompressOutput');
const decompressPassword = byId<HTMLInputElement>('decompressPassword');
const decompressResult = byId<HTMLElement>('decompressResult');

const benchmarkSource = byId<HTMLInputElement>('benchmarkSource');
const benchmarkKindTag = byId<HTMLSpanElement>('benchmarkKindTag');
const benchmarkMinLevel = byId<HTMLInputElement>('benchmarkMinLevel');
const benchmarkMaxLevel = byId<HTMLInputElement>('benchmarkMaxLevel');
const benchmarkIterations = byId<HTMLInputElement>('benchmarkIterations');
const benchmarkSampleSize = byId<HTMLInputElement>('benchmarkSampleSize');
const benchmarkSummary = byId<HTMLElement>('benchmarkSummary');
const benchmarkBars = byId<HTMLElement>('benchmarkBars');

const compressProgressBar = byId<HTMLElement>('compressProgressBar');
const compressProgressPercent = byId<HTMLElement>('compressProgressPercent');
const compressProgressText = byId<HTMLElement>('compressProgressText');
const compressProgressStats = byId<HTMLElement>('compressProgressStats');

const decompressProgressBar = byId<HTMLElement>('decompressProgressBar');
const decompressProgressPercent = byId<HTMLElement>('decompressProgressPercent');
const decompressProgressText = byId<HTMLElement>('decompressProgressText');
const decompressProgressStats = byId<HTMLElement>('decompressProgressStats');

const statusEl = byId<HTMLElement>('status');
const actionButtons = [
  byId<HTMLButtonElement>('compressSubmit'),
  byId<HTMLButtonElement>('decompressSubmit'),
  byId<HTMLButtonElement>('benchmarkSubmit')
];
const abortButtons = {
  compress: byId<HTMLButtonElement>('compressAbort'),
  decompress: byId<HTMLButtonElement>('decompressAbort'),
  benchmark: byId<HTMLButtonElement>('benchmarkAbort')
};

const themeToggle = byId<HTMLButtonElement>('themeToggle');

const previewArchive = byId<HTMLButtonElement>('previewArchive');
const archiveBrowser = byId<HTMLElement>('archiveBrowser');
const browserFileCount = byId<HTMLElement>('browserFileCount');
const browserTotalSize = byId<HTMLElement>('browserTotalSize');
const browserHash = byId<HTMLElement>('browserHash');
const browserList = byId<HTMLElement>('browserList');
const closeBrowser = byId<HTMLButtonElement>('closeBrowser');

void initProgressEvents();
void initDragAndDrop();
void initTheme();
wireEvents();

function initTheme() {
  const savedTheme = localStorage.getItem('theme') || (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  document.documentElement.setAttribute('data-theme', savedTheme);
  
  themeToggle.addEventListener('click', () => {
    const current = document.documentElement.getAttribute('data-theme');
    const next = current === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('theme', next);
  });
}

async function initDragAndDrop() {
  await listen<{ paths: string[] }>('tauri://drag-drop', (event) => {
    const paths = event.payload.paths;
    if (paths.length > 0) {
      const path = paths[0];
      // Logic: if it's a known archive format, set to decompress. Otherwise set to compress.
      const isArchive = path.toLowerCase().endsWith('.zst') || path.toLowerCase().endsWith('.enc');
      
      if (isArchive) {
        decompressSource.value = path;
        // Optionally switch tab/view if we had tabs
      } else {
        compressSource.value = path;
        benchmarkSource.value = path;
        // Update kind tag based on extension (simple heuristic)
        // Since we don't know if it's a dir here easily without invoke, we can just set the path.
      }
      setStatus(`已加载拖入的路径: ${path}`, 'success');
    }
  });

  // Visual feedback for drag over (can be added with CSS classes on body)
  await listen('tauri://drag-enter', () => {
    document.body.style.filter = 'contrast(0.9) brightness(0.9)';
  });
  await listen('tauri://drag-leave', () => {
    document.body.style.filter = '';
  });
  await listen('tauri://drag-drop', () => {
    document.body.style.filter = '';
  });
}

function wireEvents() {
  // Wire abort buttons
  abortButtons.compress.addEventListener('click', () => {
    void invoke('abort_task');
    abortButtons.compress.disabled = true;
    abortButtons.compress.textContent = '正在停止...';
  });
  abortButtons.decompress.addEventListener('click', () => {
    void invoke('abort_task');
    abortButtons.decompress.disabled = true;
    abortButtons.decompress.textContent = '正在停止...';
  });
  abortButtons.benchmark.addEventListener('click', () => {
    void invoke('abort_task');
    abortButtons.benchmark.disabled = true;
    abortButtons.benchmark.textContent = '正在停止...';
  });

  previewArchive.addEventListener('click', async () => {
    if (!decompressSource.value) {
      setStatus('请先选择归档文件。', 'error');
      return;
    }

    setBusy('正在读取归档列表...');
    try {
      const report = await invoke<ArchiveContentReport>('list_archive_content', {
        request: {
          archivePath: decompressSource.value,
          password: emptyToNull(decompressPassword.value)
        }
      });

      browserFileCount.textContent = `文件数: ${report.totalFiles}`;
      browserTotalSize.textContent = `解压大小: ${formatBytes(report.uncompressedSize)}`;
      browserHash.textContent = `BLAKE3: ${report.hash}`;
      
      browserList.innerHTML = '';
      report.entries.forEach(entry => {
        const li = document.createElement('li');
        if (entry.isDir) li.className = 'dir';
        li.innerHTML = `<span>${entry.path}</span> <small>${formatBytes(entry.size)}</small>`;
        browserList.appendChild(li);
      });

      archiveBrowser.classList.remove('hidden');
      setStatus('归档预览已加载。', 'success');
    } catch (error) {
      setStatus(normalizeError(error), 'error');
    }
  });

  closeBrowser.addEventListener('click', () => {
    archiveBrowser.classList.add('hidden');
  });

  compressLevel.addEventListener('input', () => {
    compressLevelLabel.textContent = compressLevel.value;
  });

  compressEncrypt.addEventListener('change', () => {
    compressPassword.disabled = !compressEncrypt.checked;
    if (!compressEncrypt.checked) {
      compressPassword.value = '';
    }
  });

  byId<HTMLButtonElement>('pickCompressFile').addEventListener('click', async () => {
    compressKindTag.textContent = '当前: 文件';
    const selected = await open({ title: '选择待压缩文件', multiple: false, directory: false });
    if (typeof selected === 'string') {
      compressSource.value = selected;
    }
  });

  byId<HTMLButtonElement>('pickCompressDirectory').addEventListener('click', async () => {
    compressKindTag.textContent = '当前: 目录';
    const selected = await open({ title: '选择待压缩目录', multiple: false, directory: true });
    if (typeof selected === 'string') {
      compressSource.value = selected;
    }
  });

  byId<HTMLButtonElement>('pickCompressOutput').addEventListener('click', async () => {
    const selected = await save({
      title: '压缩输出路径',
      filters: [{ name: 'Archive', extensions: ['zst', 'enc'] }]
    });
    if (typeof selected === 'string') {
      compressOutput.value = selected;
    }
  });

  byId<HTMLButtonElement>('compressSubmit').addEventListener('click', async () => {
    if (!compressSource.value) {
      setStatus('请先选择压缩源路径。', 'error');
      return;
    }

    const password = compressEncrypt.checked ? emptyToNull(compressPassword.value) : null;
    if (compressEncrypt.checked && !password) {
      setStatus('启用加密时必须输入密码。', 'error');
      return;
    }

    resetProgress('compress', '准备压缩...');

    await runTask('正在压缩，请稍候...', async () => {
      const report = await invoke<OperationReport>('compress_archive', {
        request: {
          sourcePath: compressSource.value,
          outputPath: emptyToNull(compressOutput.value),
          level: toInt(compressLevel.value, 8),
          includeRootDir: includeRootDir.checked,
          password
        }
      });
      compressResult.textContent = formatOperation(report);
      setStatus(`压缩完成: ${report.outputPath}`, 'success');
    }, 'compress');
  });

  byId<HTMLButtonElement>('pickDecompressSource').addEventListener('click', async () => {
    const selected = await open({
      title: '选择归档文件',
      multiple: false,
      directory: false,
      filters: [{ name: 'Archive', extensions: ['zst', 'enc'] }]
    });
    if (typeof selected === 'string') {
      decompressSource.value = selected;
    }
  });

  byId<HTMLButtonElement>('pickDecompressOutput').addEventListener('click', async () => {
    const selected = await open({ title: '选择解压输出目录', multiple: false, directory: true });
    if (typeof selected === 'string') {
      decompressOutput.value = selected;
    }
  });

  byId<HTMLButtonElement>('decompressSubmit').addEventListener('click', async () => {
    if (!decompressSource.value) {
      setStatus('请先选择归档文件。', 'error');
      return;
    }

    resetProgress('decompress', '准备解压...');

    await runTask('正在解压，请稍候...', async () => {
      const report = await invoke<OperationReport>('decompress_archive', {
        request: {
          archivePath: decompressSource.value,
          outputPath: emptyToNull(decompressOutput.value),
          password: emptyToNull(decompressPassword.value)
        }
      });
      decompressResult.textContent = formatOperation(report);
      setStatus(`解压完成: ${report.outputPath}`, 'success');
    }, 'decompress');
  });

  byId<HTMLButtonElement>('pickBenchmarkFile').addEventListener('click', async () => {
    benchmarkKindTag.textContent = '当前: 文件';
    const selected = await open({ title: '选择快速测试文件', multiple: false, directory: false });
    if (typeof selected === 'string') {
      benchmarkSource.value = selected;
    }
  });

  byId<HTMLButtonElement>('pickBenchmarkDirectory').addEventListener('click', async () => {
    benchmarkKindTag.textContent = '当前: 目录';
    const selected = await open({ title: '选择快速测试目录', multiple: false, directory: true });
    if (typeof selected === 'string') {
      benchmarkSource.value = selected;
    }
  });

  byId<HTMLButtonElement>('benchmarkSubmit').addEventListener('click', async () => {
    if (!benchmarkSource.value) {
      setStatus('请先选择快速测试源路径。', 'error');
      return;
    }

    await runTask('正在快速评估压缩等级...', async () => {
      const report = await invoke<CompressionBenchmarkReport>('benchmark_compression', {
        request: {
          sourcePath: benchmarkSource.value,
          minLevel: toInt(benchmarkMinLevel.value, 1),
          maxLevel: toInt(benchmarkMaxLevel.value, 12),
          iterations: toInt(benchmarkIterations.value, 2),
          sampleSizeMiB: toInt(benchmarkSampleSize.value, 64)
        }
      });
      renderBenchmark(report);
      setStatus(`测试完成，推荐压缩等级 L${report.recommendedLevel}。`, 'success');
    }, 'benchmark');
  });
}

async function initProgressEvents() {
  await listen<ProgressPayload>('zarc://progress', (event) => {
    const payload = event.payload;
    updateProgress(payload.operation, payload);

    if (payload.done && payload.error) {
      setStatus(payload.error, 'error');
    }
  });
}

function updateProgress(kind: ProgressKind, payload: ProgressPayload) {
  const refs =
    kind === 'compress'
      ? {
          bar: compressProgressBar,
          percent: compressProgressPercent,
          text: compressProgressText,
          stats: compressProgressStats
        }
      : {
          bar: decompressProgressBar,
          percent: decompressProgressPercent,
          text: decompressProgressText,
          stats: decompressProgressStats
        };

  refs.bar.style.width = `${Math.max(0, Math.min(payload.percent, 100)).toFixed(2)}%`;
  refs.percent.textContent = `${payload.percent.toFixed(1)}%`;

  if (payload.done) {
    refs.text.textContent = payload.error ? '任务失败' : '任务完成';
  } else {
    refs.text.textContent = kind === 'compress' ? '压缩进行中' : '解压进行中';
  }

  const etaText = payload.etaSeconds === null ? '-' : `${formatSeconds(payload.etaSeconds)}`;
  refs.stats.textContent =
    `已处理 ${formatBytes(payload.processedBytes)} / ${formatBytes(payload.totalBytes)} • ` +
    `速度 ${payload.throughputMiBs.toFixed(2)} MiB/s • ETA ${etaText}`;
}

function resetProgress(kind: ProgressKind, text: string) {
  updateProgress(kind, {
    operation: kind,
    processedBytes: 0,
    totalBytes: 0,
    percent: 0,
    throughputMiBs: 0,
    etaSeconds: null,
    done: false,
    error: null
  });

  if (kind === 'compress') {
    compressProgressText.textContent = text;
    compressProgressStats.textContent = '-';
  } else {
    decompressProgressText.textContent = text;
    decompressProgressStats.textContent = '-';
  }
}

function renderBenchmark(report: CompressionBenchmarkReport) {
  if (report.results.length === 0) {
    benchmarkSummary.innerHTML = '<p class="hint">未获取到可用结果。</p>';
    benchmarkBars.innerHTML = '';
    return;
  }

  const bestThroughput = Math.max(...report.results.map((r) => r.meanThroughputMiBs));
  const bestRatio = Math.min(...report.results.map((r) => r.ratioPercent));

  benchmarkSummary.innerHTML = `
    <div class="summary-grid">
      <div class="metric"><small>推荐等级</small><strong>L${report.recommendedLevel}</strong></div>
      <div class="metric"><small>样本大小</small><strong>${formatBytes(report.sampleBytes)}</strong></div>
      <div class="metric"><small>线程数</small><strong>${report.threads}</strong></div>
      <div class="metric"><small>最高吞吐</small><strong>${bestThroughput.toFixed(2)} MiB/s</strong></div>
      <div class="metric"><small>最佳压缩率</small><strong>${bestRatio.toFixed(2)}%</strong></div>
      <div class="metric"><small>每等级轮数</small><strong>${report.iterations}</strong></div>
    </div>
    <p class="hint" style="margin:10px 0 0;">${report.note}</p>
  `;

  const maxScore = Math.max(...report.results.map((r) => r.score), 1e-6);
  benchmarkBars.innerHTML = '';

  for (const row of report.results) {
    const wrap = document.createElement('div');
    wrap.className = 'bench-row';

    const label = document.createElement('small');
    label.textContent = `L${row.level}`;
    label.className = 'bench-label';

    const meter = document.createElement('div');
    meter.className = 'bench-track';

    const fill = document.createElement('div');
    fill.className = `bench-fill${row.level === report.recommendedLevel ? ' recommended' : ''}`;
    fill.style.width = `${Math.max((row.score / maxScore) * 100, 6)}%`;
    meter.append(fill);

    const info = document.createElement('small');
    info.className = 'bench-value';
    info.textContent = `${row.meanThroughputMiBs.toFixed(1)} MiB/s • ${row.ratioPercent.toFixed(2)}%`;

    wrap.append(label, meter, info);
    benchmarkBars.append(wrap);
  }
}

async function runTask(
  statusText: string,
  task: () => Promise<void>,
  kind?: 'compress' | 'decompress' | 'benchmark'
) {
  setBusy(statusText);
  setActionsDisabled(true);
  if (kind) {
    abortButtons[kind].classList.remove('hidden');
    abortButtons[kind].disabled = false;
    abortButtons[kind].textContent = '停止';
  }
  try {
    await task();
  } catch (error) {
    setStatus(normalizeError(error), 'error');
  } finally {
    setActionsDisabled(false);
    if (kind) {
      abortButtons[kind].classList.add('hidden');
    }
  }
}

function formatOperation(report: OperationReport): string {
  const ratio = report.compressionRatio === null ? '-' : `${report.compressionRatio.toFixed(2)}%`;
  const lines = [
    `操作: ${report.operation}`,
    `源路径: ${report.sourcePath}`,
    `输出路径: ${report.outputPath}`,
    `源大小: ${formatBytes(report.sourceBytes)}`,
    `结果大小: ${formatBytes(report.outputBytes)}`,
    `压缩率: ${ratio}`,
    `耗时: ${report.durationMs.toFixed(2)} ms`,
    `吞吐: ${report.throughputMiBs.toFixed(2)} MiB/s`
  ];
  if (report.blake3Hash) {
    lines.push(`BLAKE3: ${report.blake3Hash}`);
  }
  return lines.join('\n');
}

function setActionsDisabled(disabled: boolean) {
  for (const button of actionButtons) {
    button.disabled = disabled;
  }
}

function setBusy(message: string) {
  statusEl.textContent = message;
  statusEl.className = 'status busy';
}

function setStatus(message: string, level: 'success' | 'error') {
  statusEl.textContent = message;
  statusEl.className = `status ${level}`;
}

function normalizeError(error: unknown): string {
  if (typeof error === 'string') {
    return error;
  }
  if (error && typeof error === 'object' && 'toString' in error) {
    return String(error);
  }
  return '发生未知错误。';
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`无法找到元素: ${id}`);
  }
  return element as T;
}

function toInt(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return parsed;
}

function emptyToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function formatBytes(bytes: number): string {
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

function formatSeconds(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return '-';
  }
  if (seconds < 60) {
    return `${seconds.toFixed(1)}s`;
  }
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}m ${secs.toFixed(0)}s`;
}

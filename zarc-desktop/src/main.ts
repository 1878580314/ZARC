import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import './style.css';

type ProgressKind = 'compress' | 'decompress' | 'benchmark';

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
const compressSplitSize = byId<HTMLInputElement>('compressSplitSize');
const compressEncrypt = byId<HTMLInputElement>('compressEncrypt');
const compressPassword = byId<HTMLInputElement>('compressPassword');
const enableLogging = byId<HTMLInputElement>('enableLogging');
const deleteSourceAfter = byId<HTMLInputElement>('deleteSourceAfter');
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

const viewTitle = byId<HTMLElement>('viewTitle');
const globalStatus = byId<HTMLElement>('globalStatus');
const masterDropZone = byId<HTMLElement>('masterDropZone');

const taskItems = {
  compress: byId<HTMLElement>('task-compress'),
  decompress: byId<HTMLElement>('task-decompress'),
  benchmark: byId<HTMLElement>('task-benchmark')
};

const navItems = document.querySelectorAll('.nav-links li');
const viewPanels = document.querySelectorAll('.view-panel');

const themeToggle = byId<HTMLButtonElement>('themeToggle');

const previewArchive = byId<HTMLButtonElement>('previewArchive');
const archiveBrowser = byId<HTMLElement>('archiveBrowser');
const browserFileCount = byId<HTMLElement>('browserFileCount');
const browserTotalSize = byId<HTMLElement>('browserTotalSize');
const browserHash = byId<HTMLElement>('browserHash');
const browserList = byId<HTMLElement>('browserList');
const closeBrowser = byId<HTMLButtonElement>('closeBrowser');
const browserBreadcrumbs = byId<HTMLElement>('browserBreadcrumbs');

const benchmarkChartContainer = byId<HTMLElement>('benchmarkChartContainer');
const benchmarkChart = byId<HTMLElement>('benchmarkChart');

let currentArchiveEntries: ArchiveEntry[] = [];
let currentPath = '';

void initProgressEvents();
void initDragAndDrop();
void initTheme();
void initViewSwitcher();
wireEvents();

function renderBrowser() {
  browserList.innerHTML = '';
  
  // Update Breadcrumbs
  browserBreadcrumbs.innerHTML = '<span data-path="">root</span>';
  if (currentPath) {
    const parts = currentPath.split('/').filter(p => p);
    let cumulative = '';
    parts.forEach(p => {
      cumulative += p + '/';
      const span = document.createElement('span');
      span.textContent = p;
      span.dataset.path = cumulative;
      browserBreadcrumbs.appendChild(span);
    });
  }

  // Filter entries based on current path
  // Entry path is like "dir/file.txt" or "file.txt"
  const items = new Map<string, { size: number, isDir: boolean }>();
  
  currentArchiveEntries.forEach(entry => {
    const relPath = entry.path.startsWith(currentPath) ? entry.path.slice(currentPath.length) : null;
    if (relPath === null || relPath === '') return;

    const parts = relPath.split('/');
    const name = parts[0];
    const isDir = parts.length > 1 || entry.isDir;

    if (items.has(name)) {
      const existing = items.get(name)!;
      existing.size += entry.size;
      if (isDir) existing.isDir = true;
    } else {
      items.set(name, { size: entry.size, isDir });
    }
  });

  // Sort: Dirs first, then alpha
  const sortedNames = Array.from(items.keys()).sort((a, b) => {
    const ia = items.get(a)!;
    const ib = items.get(b)!;
    if (ia.isDir !== ib.isDir) return ia.isDir ? -1 : 1;
    return a.localeCompare(b);
  });

  sortedNames.forEach(name => {
    const item = items.get(name)!;
    const li = document.createElement('li');
    if (item.isDir) li.className = 'is-dir';
    
    const icon = item.isDir ? '📁' : getFileIcon(name);
    
    li.innerHTML = `
      <div class="name-wrapper">
        <span class="icon">${icon}</span>
        <span>${name}</span>
      </div>
      <span class="size">${item.isDir ? '-' : formatBytes(item.size)}</span>
    `;

    if (item.isDir) {
      li.addEventListener('click', () => {
        currentPath += name + '/';
        renderBrowser();
      });
    }
    browserList.appendChild(li);
  });
}

function getFileIcon(filename: string): string {
  const ext = filename.split('.').pop()?.toLowerCase();
  switch(ext) {
    case 'zst': case 'enc': case 'zip': case 'rar': return '📦';
    case 'exe': case 'sh': case 'app': return '⚙️';
    case 'jpg': case 'png': case 'webp': case 'gif': return '🖼️';
    case 'mp4': case 'mkv': case 'mov': return '🎬';
    case 'mp3': case 'wav': case 'flac': return '🎵';
    case 'pdf': return '📄';
    case 'txt': case 'md': return '📝';
    default: return '📄';
  }
}

function renderBenchmarkChart(report: CompressionBenchmarkReport) {
  benchmarkChartContainer.classList.remove('hidden');
  const results = report.results;
  if (results.length < 2) return;

  const margin = { top: 20, right: 40, bottom: 40, left: 50 };
  const width = benchmarkChart.clientWidth || 600;
  const height = 240;

  const maxSpeed = Math.max(...results.map(r => r.meanThroughputMiBs)) * 1.1;
  const minRatio = Math.min(...results.map(r => r.ratioPercent)) * 0.9;
  const maxRatio = Math.max(...results.map(r => r.ratioPercent)) * 1.1;

  // X: Ratio (Inverted, smaller is better/right)
  // Y: Speed (Larger is better/up)
  const getX = (ratio: number) => margin.left + (width - margin.left - margin.right) * (1 - (ratio - minRatio) / (maxRatio - minRatio));
  const getY = (speed: number) => height - margin.bottom - (height - margin.top - margin.bottom) * (speed / maxSpeed);

  let svgContent = `<svg viewBox="0 0 ${width} ${height}">`;
  
  // Axes
  svgContent += `<line x1="${margin.left}" y1="${height-margin.bottom}" x2="${width-margin.right}" y2="${height-margin.bottom}" stroke="var(--border)" stroke-width="1" />`;
  svgContent += `<line x1="${margin.left}" y1="${margin.top}" x2="${margin.left}" y2="${height-margin.bottom}" stroke="var(--border)" stroke-width="1" />`;
  
  // Axis Labels
  svgContent += `<text x="${width/2}" y="${height-5}" text-anchor="middle" class="chart-label">压缩率 % (向右越小)</text>`;
  svgContent += `<text x="10" y="${height/2}" text-anchor="middle" class="chart-label" transform="rotate(-90, 10, ${height/2})">速度 MiB/s</text>`;

  // Lines connecting points (optional, maybe not for scatter)
  // Data Points
  results.forEach(r => {
    const x = getX(r.ratioPercent);
    const y = getY(r.meanThroughputMiBs);
    const isRecommended = r.level === report.recommendedLevel;
    
    svgContent += `
      <circle cx="${x}" cy="${y}" r="${isRecommended ? 6 : 4}" 
              fill="${isRecommended ? 'var(--accent)' : 'var(--muted)'}" 
              class="chart-point ${isRecommended ? 'recommended' : ''}"
              data-level="${r.level}">
        <title>L${r.level}: ${r.meanThroughputMiBs.toFixed(1)} MiB/s, ${r.ratioPercent.toFixed(2)}%</title>
      </circle>
      <text x="${x}" y="${y-10}" text-anchor="middle" font-size="10" fill="var(--muted)">L${r.level}</text>
    `;
  });

  svgContent += '</svg>';
  benchmarkChart.innerHTML = svgContent;
}

function initViewSwitcher() {
  navItems.forEach(item => {
    item.addEventListener('click', () => {
      const view = (item as HTMLElement).dataset.view;
      if (view) switchView(view);
    });
  });
}

function switchView(viewId: string) {
  navItems.forEach(nav => {
    nav.classList.toggle('active', (nav as HTMLElement).dataset.view === viewId);
  });
  
  viewPanels.forEach(panel => {
    panel.classList.toggle('active', panel.id === `view-${viewId}`);
  });

  const titles: Record<string, string> = {
    compress: '压缩存档',
    decompress: '解压还原',
    benchmark: '性能测试'
  };
  viewTitle.textContent = titles[viewId] || 'ZARC';
}

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
    masterDropZone.classList.add('hidden');
    
    if (paths.length > 0) {
      const path = paths[0];
      const isArchive = path.toLowerCase().endsWith('.zst') || path.toLowerCase().endsWith('.enc');
      
      if (isArchive) {
        decompressSource.value = path;
        switchView('decompress');
      } else {
        compressSource.value = path;
        benchmarkSource.value = path;
        switchView('compress');
      }
      setStatus(`已加载: ${path}`, 'success');
    }
  });

  await listen('tauri://drag-enter', () => {
    masterDropZone.classList.remove('hidden');
  });
  
  await listen('tauri://drag-leave', () => {
    masterDropZone.classList.add('hidden');
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

      currentArchiveEntries = report.entries;
      currentPath = '';
      
      browserFileCount.textContent = `文件总数: ${report.totalFiles}`;
      browserTotalSize.textContent = `解压总计: ${formatBytes(report.uncompressedSize)}`;
      browserHash.textContent = `BLAKE3: ${report.hash}`;
      
      renderBrowser();
      archiveBrowser.classList.remove('hidden');
      setStatus('归档预览已加载。', 'success');
    } catch (error) {
      setStatus(normalizeError(error), 'error');
    }
  });

  browserBreadcrumbs.addEventListener('click', (e) => {
    const target = e.target as HTMLElement;
    if (target.tagName === 'SPAN' && target.dataset.path !== undefined) {
      currentPath = target.dataset.path;
      renderBrowser();
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

    resetProgress('compress');

    await runTask('正在压缩，请稍候...', async () => {
      const splitSize = toInt(compressSplitSize.value, 0);
      const report = await invoke<OperationReport>('compress_archive', {
        request: {
          sourcePath: compressSource.value,
          outputPath: emptyToNull(compressOutput.value),
          level: toInt(compressLevel.value, 8),
          includeRootDir: includeRootDir.checked,
          password,
          splitSizeMib: splitSize > 0 ? splitSize : null,
          enableLogging: enableLogging.checked,
          deleteSourceAfter: deleteSourceAfter.checked
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

    resetProgress('decompress');

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
      renderBenchmarkChart(report);
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
  const isBench = (kind as string) === 'benchmark';
  if (isBench) return; // Benchmark uses custom rendering

  const taskItem = taskItems[kind];
  const bar = byId<HTMLElement>(`${kind}ProgressBar`);
  const percent = byId<HTMLElement>(`${kind}ProgressPercent`);
  const text = byId<HTMLElement>(`${kind}ProgressText`);
  const stats = byId<HTMLElement>(`${kind}ProgressStats`);

  if (!payload.done) {
    taskItem.classList.remove('hidden');
  }

  bar.style.width = `${Math.max(0, Math.min(payload.percent, 100)).toFixed(2)}%`;
  percent.textContent = `${payload.percent.toFixed(1)}%`;

  if (payload.done) {
    text.textContent = payload.error ? '❌ 任务失败' : '✅ 任务完成';
    // Hide task from hub after a delay? Or keep it. Let's keep for now.
  } else {
    text.textContent = kind === 'compress' ? '正在压缩...' : '正在解压...';
  }

  const etaText = payload.etaSeconds === null ? '-' : `${formatSeconds(payload.etaSeconds)}`;
  stats.textContent =
    `${formatBytes(payload.processedBytes)} / ${formatBytes(payload.totalBytes)} • ` +
    `${payload.throughputMiBs.toFixed(2)} MiB/s • ETA ${etaText}`;
}

function resetProgress(kind: ProgressKind) {
  const taskItem = taskItems[kind];
  taskItem.classList.remove('hidden');
  
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
}

function renderBenchmark(report: CompressionBenchmarkReport) {
  if (report.results.length === 0) {
    benchmarkSummary.innerHTML = '<p class="hint">未获取到可用结果。</p>';
    benchmarkBars.innerHTML = '';
    return;
  }

  const bestThroughput = Math.max(...report.results.map((r) => r.meanThroughputMiBs));

  benchmarkSummary.innerHTML = `
    <div class="summary-grid">
      <div class="metric"><small>推荐等级</small><strong>L${report.recommendedLevel}</strong></div>
      <div class="metric"><small>样本大小</small><strong>${formatBytes(report.sampleBytes)}</strong></div>
      <div class="metric"><small>最高吞吐</small><strong>${bestThroughput.toFixed(2)} MiB/s</strong></div>
    </div>
    <p class="hint" style="margin:10px 0 0; font-size: 0.8rem;">${report.note}</p>
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
  globalStatus.textContent = message;
  globalStatus.className = 'global-status busy';
}

function setStatus(message: string, level: 'success' | 'error') {
  globalStatus.textContent = message;
  globalStatus.className = `global-status ${level}`;
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

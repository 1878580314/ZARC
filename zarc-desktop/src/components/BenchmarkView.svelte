<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { app } from '../stores/app.svelte';
  import { task } from '../stores/task.svelte';
  import { api } from '../lib/api';
  import type { BenchmarkReport } from '../lib/api';
  import { formatBytes } from '../lib/format';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';
  import FileInput from './ui/FileInput.svelte';
  import NumberInput from './ui/NumberInput.svelte';
  import Tag from './ui/Tag.svelte';

  let source = $state('');
  let kind = $state<'文件' | '目录'>('文件');
  let minLevel = $state(1);
  let maxLevel = $state(12);
  let iterations = $state(2);
  let sampleSize = $state(64);
  let report = $state<BenchmarkReport | null>(null);

  $effect(() => {
    if (app.benchmarkSource) {
      source = app.benchmarkSource;
      kind = app.benchmarkKind;
    }
  });

  let busy = $derived(task.busy);
  let isAbort = $derived(task.activeKind === 'benchmark' && task.aborting);
  let bestThroughput = $derived(
    report ? Math.max(...report.results.map((r) => r.meanThroughputMiBs)) : 0
  );
  let maxScore = $derived(
    report ? Math.max(...report.results.map((r) => r.score), 1e-6) : 1e-6
  );

  async function pickFile() {
    kind = '文件';
    const selected = await open({ title: '选择快速测试文件', multiple: false, directory: false });
    if (typeof selected === 'string') source = selected;
  }

  async function pickDirectory() {
    kind = '目录';
    const selected = await open({ title: '选择快速测试目录', multiple: false, directory: true });
    if (typeof selected === 'string') source = selected;
  }

  async function submit() {
    if (!source) {
      app.setStatus('请先选择快速测试源路径。', 'error');
      return;
    }
    await task.run('benchmark', '正在快速评估压缩等级...', async () => {
      const r = await api.benchmark({
        sourcePath: source,
        minLevel,
        maxLevel,
        iterations,
        sampleSizeMiB: sampleSize
      });
      report = r;
      app.setStatus(`测试完成，推荐压缩等级 L${r.recommendedLevel}。`, 'success');
    });
  }

  function abort() {
    task.requestAbort();
  }
</script>

<div class="flex flex-col gap-5 animate-[var(--animate-fade-in-scale)]">
  <Card>
    <div class="mb-5 flex items-center justify-between">
      <h3 class="text-base font-bold text-primary">性能测试配置</h3>
      <Tag>{kind}</Tag>
    </div>

    <div class="flex flex-col gap-4">
      <div class="flex flex-col gap-2">
        <span class="text-xs font-medium text-secondary">源路径</span>
        <div class="flex gap-2">
          <FileInput bind:value={source} placeholder="选择文件或目录" class="flex-1" />
          <Button variant="ghost" onclick={pickFile}>文件</Button>
          <Button variant="ghost" onclick={pickDirectory}>目录</Button>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <NumberInput bind:value={minLevel} label="最低等级" min={1} max={22} />
        <NumberInput bind:value={maxLevel} label="最高等级" min={1} max={22} />
        <NumberInput bind:value={iterations} label="每等级轮数" min={1} max={12} />
        <NumberInput bind:value={sampleSize} label="样本大小" suffix="MiB" min={4} max={1024} />
      </div>

      <div class="flex gap-3 pt-2">
        <Button onclick={submit} disabled={busy}>开始测试</Button>
        {#if task.activeKind === 'benchmark'}
          <Button variant="danger" onclick={abort} disabled={isAbort}>
            {isAbort ? '正在停止...' : '停止'}
          </Button>
        {/if}
      </div>
    </div>
  </Card>

  {#if report}
    <Card>
      <h3 class="mb-4 text-base font-bold text-primary">测试结果</h3>

      <div class="mb-5 grid grid-cols-3 gap-3">
        <div class="rounded-2xl bg-[var(--surface-hover)] p-4 text-center">
          <div class="text-xs text-muted">推荐等级</div>
          <div class="mt-1 text-2xl font-extrabold text-accent">L{report.recommendedLevel}</div>
        </div>
        <div class="rounded-2xl bg-[var(--surface-hover)] p-4 text-center">
          <div class="text-xs text-muted">样本大小</div>
          <div class="mt-1 text-2xl font-extrabold text-primary">{formatBytes(report.sampleBytes)}</div>
        </div>
        <div class="rounded-2xl bg-[var(--surface-hover)] p-4 text-center">
          <div class="text-xs text-muted">最高吞吐</div>
          <div class="mt-1 text-2xl font-extrabold text-primary">{bestThroughput.toFixed(1)}</div>
          <div class="text-[0.65rem] text-muted">MiB/s</div>
        </div>
      </div>

      {#if report.note}
        <p class="mb-4 text-xs text-muted">{report.note}</p>
      {/if}

      <div class="flex flex-col gap-2">
        {#each report.results as row (row.level)}
          {@const isRec = row.level === report.recommendedLevel}
          <div class="flex items-center gap-3">
            <span class="w-8 text-xs font-semibold {isRec ? 'text-accent' : 'text-secondary'}">L{row.level}</span>
            <div class="h-3 flex-1 overflow-hidden rounded-full bg-[var(--border-soft)]">
              <div
                class="h-full rounded-full transition-all duration-500 {isRec
                  ? 'bg-gradient-to-r from-accent to-accent-soft'
                  : 'bg-[var(--text-muted)]'}"
                style="width: {Math.max((row.score / maxScore) * 100, 6)}%"
              ></div>
            </div>
            <span class="w-36 text-right text-xs text-muted tabular-nums">
              {row.meanThroughputMiBs.toFixed(1)} MiB/s • {row.ratioPercent.toFixed(2)}%
            </span>
          </div>
        {/each}
      </div>
    </Card>
  {/if}
</div>

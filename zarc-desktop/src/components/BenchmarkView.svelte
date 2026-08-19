<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { app } from '../stores/app.svelte';
  import { task } from '../stores/task.svelte';
  import { toasts } from '../stores/toast.svelte';
  import { registerPrimaryAction } from '../lib/shortcuts';
  import { api, type BenchmarkReport } from '../lib/api';
  import { formatBytes, formatDuration, pathBaseName } from '../lib/format';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';
  import Field from './ui/Field.svelte';
  import PathInput from './ui/PathInput.svelte';
  import NumberInput from './ui/NumberInput.svelte';
  import Tag from './ui/Tag.svelte';
  import Icon from './ui/Icon.svelte';

  let minLevel = $state(1);
  let maxLevel = $state(12);
  let iterations = $state(2);
  let sampleSize = $state(64);
  let threads = $state(0);
  let report = $state<BenchmarkReport | null>(null);
  let touched = $state(false);

  // 与压缩页一样，源路径的唯一真相在 store 里。
  let source = $derived(app.benchmarkSource);
  let kind = $derived(app.benchmarkKind);

  let busy = $derived(task.busy);
  let running = $derived(task.activeKind === 'benchmark');

  let sourceError = $derived(touched && !source ? '请先选择用于测试的文件或目录。' : undefined);
  let rangeError = $derived(minLevel > maxLevel ? '最低等级不能高于最高等级。' : undefined);

  let estimatedRuns = $derived(Math.max(0, maxLevel - minLevel + 1) * iterations);

  // 分数、吞吐、体积各自的极值，用来把柱状图归一化。
  let bestThroughput = $derived(
    report && report.results.length > 0
      ? Math.max(...report.results.map((r) => r.meanThroughputMiBs))
      : 0
  );
  let bestRatio = $derived(
    report && report.results.length > 0
      ? Math.min(...report.results.map((r) => r.ratioPercent))
      : 0
  );
  let worstRatio = $derived(
    report && report.results.length > 0
      ? Math.max(...report.results.map((r) => r.ratioPercent))
      : 0
  );

  $effect(() => registerPrimaryAction('benchmark', submit));

  async function pickFile(): Promise<void> {
    const selected = await open({ title: '选择测试文件', multiple: false, directory: false });
    if (typeof selected === 'string') app.setBenchmarkSource(selected, '文件');
  }

  async function pickDirectory(): Promise<void> {
    const selected = await open({ title: '选择测试目录', multiple: false, directory: true });
    if (typeof selected === 'string') app.setBenchmarkSource(selected, '目录');
  }

  async function submit(): Promise<void> {
    touched = true;
    if (!source) {
      app.setStatus('请先选择测试源路径。', 'error');
      toasts.warn('还没有选择源路径', '挑一个有代表性的样本，结果才有参考价值。');
      return;
    }
    if (rangeError) {
      app.setStatus(rangeError, 'error');
      return;
    }

    const ok = await task.run('benchmark', `正在评估 ${pathBaseName(source)}...`, async () => {
      report = await api.benchmark({
        sourcePath: source,
        minLevel,
        maxLevel,
        iterations,
        sampleSizeMib: sampleSize,
        threads: threads > 0 ? threads : null
      });
      app.setStatus(`测试完成，推荐压缩等级 L${report.recommendedLevel}。`, 'success');
    });

    if (ok && report) {
      toasts.success('测试完成', `推荐等级 L${report.recommendedLevel}，可一键应用到压缩页。`);
    }
  }

  /** 体积条：越短越好，所以按「相对最差值」缩放，视觉上短即优。 */
  function ratioWidth(value: number): number {
    if (worstRatio <= 0) return 6;
    return Math.max((value / worstRatio) * 100, 6);
  }

  function throughputWidth(value: number): number {
    if (bestThroughput <= 0) return 6;
    return Math.max((value / bestThroughput) * 100, 6);
  }
</script>

<div class="flex flex-col gap-4 animate-[var(--animate-rise)]">
  <Card title="测试设置" subtitle="在样本上逐级试跑，找出速度与体积的平衡点" icon="benchmark">
    {#snippet actions()}
      <Tag tone={source ? 'accent' : 'neutral'}>{source ? kind : '未选择'}</Tag>
    {/snippet}

    <div class="flex flex-col gap-4">
      <Field label="测试源" error={sourceError}>
        <PathInput
          value={source}
          onCommit={(next) => app.setBenchmarkSource(next, kind)}
          icon={kind === '目录' ? 'folder' : 'file'}
          invalid={Boolean(sourceError)}
          placeholder="挑一个有代表性的文件或目录"
        >
          {#snippet actions()}
            <Button variant="ghost" size="sm" icon="file" onclick={pickFile}>文件</Button>
            <Button variant="ghost" size="sm" icon="folder" onclick={pickDirectory}>目录</Button>
          {/snippet}
        </PathInput>
      </Field>

      <div class="grid grid-cols-2 gap-4">
        <Field label="最低等级" error={rangeError}>
          <NumberInput bind:value={minLevel} min={1} max={22} />
        </Field>
        <Field label="最高等级">
          <NumberInput bind:value={maxLevel} min={1} max={22} />
        </Field>
        <Field label="每级轮数" hint="多轮取均值可以削平系统抖动。">
          <NumberInput bind:value={iterations} min={1} max={12} />
        </Field>
        <Field label="样本大小" hint="只取前若干 MiB 参与测试。">
          <NumberInput bind:value={sampleSize} suffix="MiB" min={4} max={1024} />
        </Field>
      </div>

      <Field label="工作线程" hint="0 表示使用全部可用核心；固定线程数便于横向对比。">
        <NumberInput bind:value={threads} suffix="线程" min={0} max={256} />
      </Field>

      <div class="flex items-center gap-2 rounded-control bg-inset px-3 py-2 text-xs text-fg-faint">
        <Icon name="info" size={14} />
        <span>
          共需 {estimatedRuns} 次压缩，高等级耗时会显著拉长；测试期间只读取样本，不写入磁盘。
        </span>
      </div>

      <div class="flex items-center gap-3 border-t border-line pt-4">
        <Button
          icon="play"
          loading={running}
          disabled={(busy && !running) || Boolean(rangeError)}
          onclick={submit}
        >
          开始测试
        </Button>
        {#if running}
          <Button
            variant="danger"
            icon="stop"
            disabled={task.aborting}
            onclick={() => task.requestAbort()}
          >
            {task.aborting ? '正在停止' : '停止'}
          </Button>
        {/if}
        <span class="ml-auto text-[0.7rem] text-fg-faint">Ctrl + Enter</span>
      </div>
    </div>
  </Card>

  {#if report}
    <!-- snippet 是独立闭包，`{#if report}` 的收窄传不进去，这里先固定一份引用。 -->
    {@const rep = report}
    <Card title="测试结果" subtitle={pathBaseName(rep.sourcePath)} icon="checkCircle">
      {#snippet actions()}
        <Button
          size="sm"
          icon="check"
          onclick={() => app.applyRecommendedLevel(rep.recommendedLevel)}
        >
          采用 L{rep.recommendedLevel}
        </Button>
        <Button variant="subtle" size="sm" icon="close" onclick={() => (report = null)}>
          关闭
        </Button>
      {/snippet}

      <div class="flex flex-col gap-4">
        <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <div class="rounded-control bg-accent-wash px-3 py-3 text-center">
            <div class="text-[0.7rem] text-fg-faint">推荐等级</div>
            <div class="mt-1 text-xl font-extrabold text-accent tabular-nums">
              L{rep.recommendedLevel}
            </div>
          </div>
          <div class="rounded-control bg-inset px-3 py-3 text-center">
            <div class="text-[0.7rem] text-fg-faint">样本大小</div>
            <div class="mt-1 text-xl font-extrabold text-fg tabular-nums">
              {formatBytes(rep.sampleBytes)}
            </div>
          </div>
          <div class="rounded-control bg-inset px-3 py-3 text-center">
            <div class="text-[0.7rem] text-fg-faint">最高吞吐</div>
            <div class="mt-1 text-xl font-extrabold text-fg tabular-nums">
              {bestThroughput.toFixed(0)}
              <span class="text-xs font-medium text-fg-faint">MiB/s</span>
            </div>
          </div>
          <div class="rounded-control bg-inset px-3 py-3 text-center">
            <div class="text-[0.7rem] text-fg-faint">最优体积</div>
            <div class="mt-1 text-xl font-extrabold text-fg tabular-nums">
              {bestRatio.toFixed(1)}<span class="text-xs font-medium text-fg-faint">%</span>
            </div>
          </div>
        </div>

        {#if rep.note}
          <p class="flex items-start gap-2 rounded-control bg-inset px-3 py-2 text-xs leading-relaxed text-fg-soft">
            <Icon name="info" size={14} class="mt-px shrink-0 text-fg-faint" />
            <span>{rep.note}</span>
          </p>
        {/if}

        <div class="flex flex-col gap-1">
          <div class="flex items-center gap-3 px-2 text-[0.65rem] tracking-wide text-fg-faint">
            <span class="w-9">等级</span>
            <span class="flex-1">压缩后体积（越短越好）</span>
            <span class="flex-1">吞吐（越长越快）</span>
            <span class="w-16 text-right">耗时</span>
          </div>

          {#each rep.results as row (row.level)}
            {@const best = row.level === rep.recommendedLevel}
            <div
              class="flex items-center gap-3 rounded-control px-2 py-2 transition-colors {best
                ? 'bg-accent-wash'
                : 'hover:bg-inset'}"
            >
              <span
                class="w-9 text-xs font-bold tabular-nums {best ? 'text-accent' : 'text-fg-soft'}"
              >
                L{row.level}
              </span>

              <div class="flex flex-1 items-center gap-2">
                <div class="h-2 flex-1 overflow-hidden rounded-pill bg-inset-strong">
                  <div
                    class="h-full rounded-pill transition-[width] duration-500 {best
                      ? 'bg-accent'
                      : 'bg-fg-faint'}"
                    style="width: {ratioWidth(row.ratioPercent)}%"
                  ></div>
                </div>
                <span class="w-12 shrink-0 text-right text-[0.7rem] text-fg-faint tabular-nums">
                  {row.ratioPercent.toFixed(1)}%
                </span>
              </div>

              <div class="flex flex-1 items-center gap-2">
                <div class="h-2 flex-1 overflow-hidden rounded-pill bg-inset-strong">
                  <div
                    class="h-full rounded-pill bg-success/70 transition-[width] duration-500"
                    style="width: {throughputWidth(row.meanThroughputMiBs)}%"
                  ></div>
                </div>
                <span class="w-16 shrink-0 text-right text-[0.7rem] text-fg-faint tabular-nums">
                  {row.meanThroughputMiBs.toFixed(0)} MiB/s
                </span>
              </div>

              <span class="w-16 text-right text-[0.7rem] text-fg-faint tabular-nums">
                {formatDuration(row.meanMs)}
              </span>
            </div>
          {/each}
        </div>
      </div>
    </Card>
  {/if}
</div>

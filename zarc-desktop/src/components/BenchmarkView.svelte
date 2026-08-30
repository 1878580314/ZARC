<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { app } from '../stores/app.svelte';
  import { task } from '../stores/task.svelte';
  import { toasts } from '../stores/toast.svelte';
  import { registerPrimaryAction } from '../lib/shortcuts';
  import { api, type BenchmarkReport } from '../lib/api';
  import { formatBytes, formatDuration, pathBaseName } from '../lib/format';
  import { t } from '../lib/i18n/index.svelte';
  import { translateBackendText } from '../lib/i18n/backend';
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

  // 与压缩视图一致，store 是源路径的唯一真源。 / Like the Compress view, the store is the single source of truth for the source path.
  let source = $derived(app.benchmarkSource);
  let kind = $derived(app.benchmarkKind);

  let busy = $derived(task.busy);
  let running = $derived(task.activeKind === 'benchmark');

  let sourceError = $derived(touched && !source ? t('benchmark.error.noSource') : undefined);
  let rangeError = $derived(minLevel > maxLevel ? t('benchmark.error.range') : undefined);

  let estimatedRuns = $derived(Math.max(0, maxLevel - minLevel + 1) * iterations);

  // 得分/吞吐/大小的极值，用于归一化条形图。 / Extremes of score, throughput, and size, used to normalize the bars.
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
    const selected = await open({ title: t('benchmark.dialog.pickFile'), multiple: false, directory: false });
    if (typeof selected === 'string') app.setBenchmarkSource(selected, 'file');
  }

  async function pickDirectory(): Promise<void> {
    const selected = await open({ title: t('benchmark.dialog.pickFolder'), multiple: false, directory: true });
    if (typeof selected === 'string') app.setBenchmarkSource(selected, 'folder');
  }

  async function submit(): Promise<void> {
    touched = true;
    if (!source) {
      app.setStatus(t('benchmark.error.sourceRequired'), 'error');
      toasts.warn(t('toast.noSource'), t('benchmark.hint.noSource'));
      return;
    }
    if (rangeError) {
      app.setStatus(rangeError, 'error');
      return;
    }

    const ok = await task.run('benchmark', t('benchmark.running', { name: pathBaseName(source) }), async () => {
      report = await api.benchmark({
        sourcePath: source,
        minLevel,
        maxLevel,
        iterations,
        sampleSizeMib: sampleSize,
        threads: threads > 0 ? threads : null
      });
      app.setStatus(t('benchmark.status.done', { level: report.recommendedLevel }), 'success');
    });

    if (ok && report) {
      toasts.success(t('toast.benchmarkComplete'), t('benchmark.hint.recommended', { level: report.recommendedLevel }));
    }
  }

  /** 大小条形：越短越好，故按最差值缩放。 / Size bars: shorter is better, so they are scaled against the worst value - visually, shorter means better. */
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
  <Card title={t('benchmark.settingsCard.title')} subtitle={t('benchmark.settingsCard.subtitle')} icon="benchmark">
    {#snippet actions()}
      <Tag tone={source ? 'accent' : 'neutral'}>{source ? t(kind === 'folder' ? 'kind.folder' : 'kind.file') : t('notSelected')}</Tag>
    {/snippet}

    <div class="flex flex-col gap-4">
      <Field label={t('benchmark.source')} error={sourceError}>
        <PathInput
          value={source}
          onCommit={(next) => app.setBenchmarkSource(next, kind)}
          icon={kind === 'folder' ? 'folder' : 'file'}
          invalid={Boolean(sourceError)}
          placeholder={t('benchmark.sourcePlaceholder')}
        >
          {#snippet actions()}
            <Button variant="ghost" size="sm" icon="file" onclick={pickFile}>{t('kind.file')}</Button>
            <Button variant="ghost" size="sm" icon="folder" onclick={pickDirectory}>{t('kind.folder')}</Button>
          {/snippet}
        </PathInput>
      </Field>

      <div class="grid grid-cols-2 gap-4">
        <Field label={t('benchmark.minLevel')} error={rangeError}>
          <NumberInput bind:value={minLevel} min={1} max={22} />
        </Field>
        <Field label={t('benchmark.maxLevel')}>
          <NumberInput bind:value={maxLevel} min={1} max={22} />
        </Field>
        <Field label={t('benchmark.iterations')} hint={t('benchmark.hint.iterations')}>
          <NumberInput bind:value={iterations} min={1} max={12} />
        </Field>
        <Field label={t('benchmark.sampleSize')} hint={t('benchmark.hint.sampleSize')}>
          <NumberInput bind:value={sampleSize} suffix="MiB" min={4} max={1024} />
        </Field>
      </div>

      <Field label={t('benchmark.threads')} hint={t('benchmark.threadsHint')}>
        <NumberInput bind:value={threads} suffix={t('benchmark.threadsSuffix')} min={0} max={256} />
      </Field>

      <div class="flex items-center gap-2 rounded-control bg-inset px-3 py-2 text-xs text-fg-faint">
        <Icon name="info" size={14} />
        <span>
          {t('benchmark.runsInfo', { runs: estimatedRuns })}
        </span>
      </div>

      <div class="flex items-center gap-3 border-t border-line pt-4">
        <Button
          icon="play"
          loading={running}
          disabled={(busy && !running) || Boolean(rangeError)}
          onclick={submit}
        >
          {t('benchmark.submit')}
        </Button>
        {#if running}
          <Button
            variant="danger"
            icon="stop"
            disabled={task.aborting}
            onclick={() => task.requestAbort()}
          >
            {task.aborting ? t('task.stopping') : t('task.stop')}
          </Button>
        {/if}
        <span class="ml-auto text-[0.7rem] text-fg-faint">Ctrl + Enter</span>
      </div>
    </div>
  </Card>

  {#if report}
    <!-- Snippet 是独立闭包，`{#if report}` 的类型收窄无法传入；先固定为局部引用。 / Snippets are independent closures, so the `{#if report}` narrowing can't reach
         inside; pin a local reference first. -->
    {@const rep = report}
    <Card title={t('benchmark.resultsCard.title')} subtitle={pathBaseName(rep.sourcePath)} icon="checkCircle">
      {#snippet actions()}
        <Button
          size="sm"
          icon="check"
          onclick={() => app.applyRecommendedLevel(rep.recommendedLevel)}
        >
          {t('benchmark.useLevel', { level: rep.recommendedLevel })}
        </Button>
        <Button variant="subtle" size="sm" icon="close" onclick={() => (report = null)}>
          {t('benchmark.close')}
        </Button>
      {/snippet}

      <div class="flex flex-col gap-4">
        <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <div class="rounded-control bg-accent-wash px-3 py-3 text-center">
            <div class="text-[0.7rem] text-fg-faint">{t('benchmark.recommendedLevel')}</div>
            <div class="mt-1 text-xl font-extrabold text-accent tabular-nums">
              L{rep.recommendedLevel}
            </div>
          </div>
          <div class="rounded-control bg-inset px-3 py-3 text-center">
            <div class="text-[0.7rem] text-fg-faint">{t('benchmark.sampleSize')}</div>
            <div class="mt-1 text-xl font-extrabold text-fg tabular-nums">
              {formatBytes(rep.sampleBytes)}
            </div>
          </div>
          <div class="rounded-control bg-inset px-3 py-3 text-center">
            <div class="text-[0.7rem] text-fg-faint">{t('benchmark.peakThroughput')}</div>
            <div class="mt-1 text-xl font-extrabold text-fg tabular-nums">
              {bestThroughput.toFixed(0)}
              <span class="text-xs font-medium text-fg-faint">MiB/s</span>
            </div>
          </div>
          <div class="rounded-control bg-inset px-3 py-3 text-center">
            <div class="text-[0.7rem] text-fg-faint">{t('benchmark.bestRatio')}</div>
            <div class="mt-1 text-xl font-extrabold text-fg tabular-nums">
              {bestRatio.toFixed(1)}<span class="text-xs font-medium text-fg-faint">%</span>
            </div>
          </div>
        </div>

        {#if rep.note}
          <p class="flex items-start gap-2 rounded-control bg-inset px-3 py-2 text-xs leading-relaxed text-fg-soft">
            <Icon name="info" size={14} class="mt-px shrink-0 text-fg-faint" />
            <span>{translateBackendText(rep.note)}</span>
          </p>
        {/if}

        <div class="flex flex-col gap-1">
          <div class="flex items-center gap-3 px-2 text-[0.65rem] tracking-wide text-fg-faint">
            <span class="w-9">{t('benchmark.col.level')}</span>
            <span class="flex-1">{t('benchmark.col.size')}</span>
            <span class="flex-1">{t('benchmark.col.throughput')}</span>
            <span class="w-16 text-right">{t('field.elapsed')}</span>
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

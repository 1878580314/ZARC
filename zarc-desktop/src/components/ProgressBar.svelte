<script lang="ts">
  import type { TaskProgress } from '../stores/progress.svelte';
  import { formatBytes, formatSeconds } from '../lib/format';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './ui/Icon.svelte';

  interface Props {
    progress: TaskProgress;
    /** Description shown while running; falls back to the task label recorded in the store when empty. */
    label?: string;
    compact?: boolean;
  }

  let { progress, label, compact = false }: Props = $props();

  let running = $derived(progress.label || label || t('shell.processing'));
  let statusText = $derived(
    progress.done ? (progress.error ? t('taskFailed') : t('taskComplete')) : running
  );
  let width = $derived(`${Math.max(0, Math.min(progress.percent, 100)).toFixed(2)}%`);
  let etaText = $derived(progress.etaSeconds === null ? '—' : formatSeconds(progress.etaSeconds));

  /**
   * totalBytes is still 0 until the first progress event arrives from the
   * backend. Show an indeterminate back-and-forth bar rather than a dead
   * bar stuck at 0%.
   */
  let indeterminate = $derived(!progress.done && (!progress.started || progress.totalBytes === 0));

  let barTone = $derived(
    progress.error ? 'bg-danger' : progress.done ? 'bg-success' : 'bg-accent'
  );
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-baseline justify-between gap-2 text-xs">
    <span class="flex min-w-0 items-center gap-1.5 font-medium text-fg-soft">
      {#if progress.done}
        <Icon
          name={progress.error ? 'error' : 'checkCircle'}
          size={13}
          class={progress.error ? 'text-danger' : 'text-success'}
        />
      {/if}
      <span class="truncate">{statusText}</span>
    </span>
    {#if !indeterminate}
      <span class="shrink-0 font-semibold text-fg tabular-nums">
        {progress.percent.toFixed(1)}%
      </span>
    {/if}
  </div>

  <div class="relative h-1.5 w-full overflow-hidden rounded-pill bg-inset-strong">
    {#if indeterminate}
      <div class="absolute inset-y-0 animate-[var(--animate-indeterminate)] rounded-pill bg-accent"></div>
    {:else}
      <div
        class="absolute inset-y-0 left-0 overflow-hidden rounded-pill transition-[width] duration-300 ease-out {barTone}"
        style="width: {width}"
      >
        {#if !progress.done}
          <span class="sheen absolute inset-0"></span>
        {/if}
      </div>
    {/if}
  </div>

  {#if progress.error}
    <p class="line-clamp-3 text-[0.7rem] leading-relaxed break-words text-danger" data-selectable>
      {progress.error}
    </p>
  {:else if !compact}
    <div class="flex items-center gap-1.5 text-[0.7rem] text-fg-faint tabular-nums">
      <span>{formatBytes(progress.processedBytes)}</span>
      <span class="opacity-50">/</span>
      <span>{progress.totalBytes > 0 ? formatBytes(progress.totalBytes) : '—'}</span>
      <span class="opacity-40">·</span>
      <span>{progress.throughputMiBs.toFixed(1)} MiB/s</span>
      {#if !progress.done}
        <span class="opacity-40">·</span>
        <span>{t('shell.etaRemaining', { eta: etaText })}</span>
      {/if}
    </div>
  {/if}
</div>

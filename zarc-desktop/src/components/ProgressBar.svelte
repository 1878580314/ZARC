<script lang="ts">
  import type { TaskProgress } from '../stores/progress.svelte';
  import { formatBytes, formatSeconds } from '../lib/format';

  interface Props {
    progress: TaskProgress;
    label: string;
  }

  let { progress, label }: Props = $props();

  let percent = $derived(progress.percent.toFixed(1));
  let width = $derived(`${Math.max(0, Math.min(progress.percent, 100)).toFixed(2)}%`);
  let etaText = $derived(progress.etaSeconds === null ? '-' : formatSeconds(progress.etaSeconds));
  let statusText = $derived(
    progress.done
      ? progress.error
        ? '任务失败'
        : '任务完成'
      : label
  );
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-center justify-between text-xs">
    <span class="font-medium text-secondary">{statusText}</span>
    <span class="font-semibold text-primary tabular-nums">{percent}%</span>
  </div>
  <div class="relative h-2.5 w-full overflow-hidden rounded-full bg-[var(--border-soft)]">
    {#if progress.done}
      <div
        class="absolute inset-y-0 left-0 rounded-full transition-all duration-500 {progress.error
          ? 'bg-danger'
          : 'bg-success'}"
        style="width: {width}"
      ></div>
    {:else}
      <div
        class="absolute inset-y-0 left-0 rounded-full bg-gradient-to-r from-accent to-accent-soft transition-[width] duration-300 ease-out"
        style="width: {width}"
      >
        <div class="shimmer-bg absolute inset-0 opacity-40"></div>
      </div>
    {/if}
  </div>
  <div class="text-[0.7rem] text-muted tabular-nums">
    {formatBytes(progress.processedBytes)} / {formatBytes(progress.totalBytes)} •
    {progress.throughputMiBs.toFixed(2)} MiB/s • ETA {etaText}
  </div>
</div>

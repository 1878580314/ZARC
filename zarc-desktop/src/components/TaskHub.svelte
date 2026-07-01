<script lang="ts">
  import { progress } from '../stores/progress.svelte';
  import { task } from '../stores/task.svelte';
  import ProgressBar from './ProgressBar.svelte';

  let compress = $derived(progress.compress);
  let decompress = $derived(progress.decompress);
  let benchRunning = $derived(task.activeKind === 'benchmark');
</script>

<aside class="flex h-full w-80 shrink-0 flex-col gap-3 p-4">
  <div class="px-2 pt-2">
    <h2 class="text-xs font-bold tracking-wider text-muted uppercase">任务中心</h2>
  </div>

  <div class="glass flex flex-col gap-3 rounded-[var(--radius-card)] p-4">
    <div class="flex items-center gap-2 text-sm font-semibold text-primary">
      <span>🗜️</span> 压缩
    </div>
    {#if compress.visible}
      <ProgressBar progress={compress} label="正在压缩..." />
    {:else}
      <p class="text-xs text-muted">空闲</p>
    {/if}
  </div>

  <div class="glass flex flex-col gap-3 rounded-[var(--radius-card)] p-4">
    <div class="flex items-center gap-2 text-sm font-semibold text-primary">
      <span>📂</span> 解压
    </div>
    {#if decompress.visible}
      <ProgressBar progress={decompress} label="正在解压..." />
    {:else}
      <p class="text-xs text-muted">空闲</p>
    {/if}
  </div>

  <div class="glass flex flex-col gap-3 rounded-[var(--radius-card)] p-4">
    <div class="flex items-center gap-2 text-sm font-semibold text-primary">
      <span>⚡</span> 测试
    </div>
    {#if benchRunning}
      <div class="flex items-center gap-2 text-xs text-secondary">
        <span
          class="h-3 w-3 animate-[var(--animate-spin-slow)] rounded-full border-2 border-accent border-t-transparent"
        ></span>
        正在运行...
      </div>
    {:else}
      <p class="text-xs text-muted">空闲</p>
    {/if}
  </div>
</aside>

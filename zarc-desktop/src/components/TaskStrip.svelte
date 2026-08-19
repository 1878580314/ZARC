<script lang="ts">
  import { slide } from 'svelte/transition';
  import { progress } from '../stores/progress.svelte';
  import { task } from '../stores/task.svelte';
  import ProgressBar from './ProgressBar.svelte';
  import Button from './ui/Button.svelte';
  import Spinner from './ui/Spinner.svelte';

  // 窄窗口下侧栏任务中心被收起，进度必须在主列有个落脚点。
  let bars = $derived(
    [
      { key: 'compress', value: progress.compress },
      { key: 'decompress', value: progress.decompress }
    ].filter((s) => s.value.visible)
  );

  let benchRunning = $derived(task.activeKind === 'benchmark');
  let show = $derived(bars.length > 0 || benchRunning);
</script>

{#if show}
  <div class="panel flex flex-col gap-3 rounded-panel px-4 py-3" transition:slide={{ duration: 180 }}>
    {#each bars as bar (bar.key)}
      <ProgressBar progress={bar.value} compact />
    {/each}

    {#if benchRunning}
      <div class="flex items-center gap-2 text-xs font-medium text-accent">
        <Spinner size={13} />
        正在逐级试跑压缩等级...
      </div>
    {/if}

    {#if task.busy}
      <Button
        variant="danger"
        size="sm"
        icon="stop"
        class="self-start"
        loading={task.aborting}
        onclick={() => task.requestAbort()}
      >
        {task.aborting ? '正在停止' : '停止任务'}
      </Button>
    {/if}
  </div>
{/if}

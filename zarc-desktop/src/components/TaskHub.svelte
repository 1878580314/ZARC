<script lang="ts">
  import { slide } from 'svelte/transition';
  import { progress, type TaskProgress } from '../stores/progress.svelte';
  import { task } from '../stores/task.svelte';
  import type { ProgressKind } from '../lib/api';
  import ProgressBar from './ProgressBar.svelte';
  import Icon, { type IconName } from './ui/Icon.svelte';
  import Spinner from './ui/Spinner.svelte';
  import Button from './ui/Button.svelte';

  interface Slot {
    kind: ProgressKind;
    label: string;
    icon: IconName;
    progress: TaskProgress | null;
  }

  let slots = $derived<Slot[]>([
    { kind: 'compress', label: '压缩', icon: 'compress', progress: progress.compress },
    { kind: 'decompress', label: '解压', icon: 'decompress', progress: progress.decompress },
    { kind: 'benchmark', label: '测试', icon: 'benchmark', progress: null }
  ]);

  let running = $derived(task.activeKind);

  /**
   * 状态归纳成一个词，让折叠态一眼可读。
   * benchmark 没有进度槽位，只有「运行中 / 空闲」两态。
   */
  function summary(slot: Slot): { text: string; tone: string } {
    if (running === slot.kind) return { text: '运行中', tone: 'text-accent' };
    if (!slot.progress?.visible) return { text: '空闲', tone: 'text-fg-faint' };
    if (slot.progress.error) return { text: '失败', tone: 'text-danger' };
    if (slot.progress.done) return { text: '已完成', tone: 'text-success' };
    return { text: '待机', tone: 'text-fg-faint' };
  }

  /** 折叠态只保留标题行；展开态在运行中或有终态结果时出现。 */
  function expanded(slot: Slot): boolean {
    return running === slot.kind || Boolean(slot.progress?.visible);
  }
</script>

<!-- 窗口不够宽时收起整条侧栏，改由主列的 TaskStrip 顶上。 -->
<aside class="hidden h-full w-[18rem] shrink-0 flex-col gap-3 py-4 pr-4 pl-1 min-[1180px]:flex">
  <div class="flex items-center justify-between px-2">
    <h2 class="text-[0.68rem] font-bold tracking-[0.12em] text-fg-faint uppercase">任务中心</h2>
    {#if task.busy}
      <span class="flex items-center gap-1.5 text-[0.68rem] font-medium text-accent">
        <span class="h-1.5 w-1.5 animate-[var(--animate-breathe)] rounded-full bg-accent"></span>
        运行中
      </span>
    {/if}
  </div>

  {#each slots as slot (slot.kind)}
    {@const state = summary(slot)}
    {@const active = running === slot.kind}
    <section
      class="panel flex flex-col gap-3 rounded-panel p-4 transition-shadow duration-300 {active
        ? 'shadow-[var(--shadow-glow)] ring-1 ring-accent/35'
        : ''}"
    >
      <div class="flex items-center gap-2.5">
        <span
          class="flex h-7 w-7 items-center justify-center rounded-control transition-colors {active
            ? 'bg-accent text-accent-fg'
            : 'bg-inset text-fg-soft'}"
        >
          {#if active}
            <Spinner size={14} />
          {:else}
            <Icon name={slot.icon} size={15} />
          {/if}
        </span>
        <span class="flex-1 text-sm font-semibold text-fg">{slot.label}</span>
        <span class="text-[0.7rem] font-medium {state.tone}">{state.text}</span>
      </div>

      {#if slot.progress && expanded(slot)}
        <div transition:slide={{ duration: 180 }}>
          <ProgressBar progress={slot.progress} label="正在{slot.label}..." />
        </div>
      {:else if active}
        <p class="text-[0.7rem] text-fg-faint">正在逐级试跑，完成后会给出推荐等级。</p>
      {/if}
    </section>
  {/each}

  {#if task.busy}
    <div transition:slide={{ duration: 180 }}>
      <Button
        variant="danger"
        size="sm"
        icon="stop"
        class="w-full"
        loading={task.aborting}
        onclick={() => task.requestAbort()}
      >
        {task.aborting ? '正在停止' : '停止任务'}
      </Button>
      <p class="mt-2 text-center text-[0.65rem] text-fg-faint">按 Esc 也可以中止</p>
    </div>
  {/if}

  <div class="mt-auto px-2 pt-2">
    <p class="text-[0.65rem] leading-relaxed text-fg-faint">
      把文件拖到窗口任意位置即可快速载入；归档会自动进入解压页。
    </p>
  </div>
</aside>

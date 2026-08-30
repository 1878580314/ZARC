<script lang="ts">
  import { slide } from 'svelte/transition';
  import { progress, type TaskProgress } from '../stores/progress.svelte';
  import { task } from '../stores/task.svelte';
  import type { ProgressKind } from '../lib/api';
  import { t } from '../lib/i18n/index.svelte';
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
    { kind: 'compress', label: t('nav.compress'), icon: 'compress', progress: progress.compress },
    { kind: 'decompress', label: t('nav.extract'), icon: 'decompress', progress: progress.decompress },
    { kind: 'benchmark', label: t('nav.benchmark'), icon: 'benchmark', progress: null }
  ]);

  let running = $derived(task.activeKind);

  /**
   * 把状态蒸馏成一个词，折叠卡片一眼可读。基准测试没有进度槽，只有「运行/空闲」两态。
   * Distill the state into a single word so the collapsed card reads at a glance.
   * Benchmark has no progress slot, only the "running / idle" pair.
   */
  function summary(slot: Slot): { text: string; tone: string } {
    if (running === slot.kind) return { text: t('shell.running'), tone: 'text-accent' };
    if (!slot.progress?.visible) return { text: t('shell.idle'), tone: 'text-fg-faint' };
    if (slot.progress.error) return { text: t('shell.failed'), tone: 'text-danger' };
    if (slot.progress.done) return { text: t('shell.done'), tone: 'text-success' };
    return { text: t('shell.pending'), tone: 'text-fg-faint' };
  }

  /** 折叠卡片只留标题行；运行中或持有最终结果时展开。 / Collapsed cards keep only the header row; expanded while running or holding a final result. */
  function expanded(slot: Slot): boolean {
    return running === slot.kind || Boolean(slot.progress?.visible);
  }
</script>

<!-- 窗口过窄时隐藏；TaskStrip 接管主列顶部。 / Hidden when the window is too narrow; TaskStrip takes over at the top of the main column. -->
<aside class="hidden h-full w-[18rem] shrink-0 flex-col gap-3 py-4 pr-4 pl-1 min-[1180px]:flex">
  <div class="flex items-center justify-between px-2">
    <h2 class="text-[0.68rem] font-bold tracking-[0.12em] text-fg-faint uppercase">{t('shell.taskHub')}</h2>
    {#if task.busy}
      <span class="flex items-center gap-1.5 text-[0.68rem] font-medium text-accent">
        <span class="h-1.5 w-1.5 animate-[var(--animate-breathe)] rounded-full bg-accent"></span>
        {t('shell.running')}
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
          <ProgressBar progress={slot.progress} label={t('shell.working', { label: slot.label })} />
        </div>
      {:else if active}
        <p class="text-[0.7rem] text-fg-faint">{t('shell.benchmarkHint')}</p>
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
        {task.aborting ? t('task.stopping') : t('task.stop')}
      </Button>
      <p class="mt-2 text-center text-[0.65rem] text-fg-faint">{t('shell.escToAbort')}</p>
    </div>
  {/if}

  <div class="mt-auto px-2 pt-2">
    <p class="text-[0.65rem] leading-relaxed text-fg-faint">
      {t('shell.dropHint')}
    </p>
  </div>
</aside>

<script lang="ts">
  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { theme } from './stores/theme.svelte';
  import { app } from './stores/app.svelte';
  import { initProgressListener } from './lib/progress';
  import { initDragDrop } from './lib/dragdrop.svelte';
  import { initShortcuts } from './lib/shortcuts';
  import Sidebar from './components/Sidebar.svelte';
  import TopBar from './components/TopBar.svelte';
  import TaskHub from './components/TaskHub.svelte';
  import TaskStrip from './components/TaskStrip.svelte';
  import MasterDropZone from './components/MasterDropZone.svelte';
  import ToastHost from './components/ToastHost.svelte';
  import ShortcutsDialog from './components/ShortcutsDialog.svelte';
  import CompressView from './components/CompressView.svelte';
  import DecompressView from './components/DecompressView.svelte';
  import BenchmarkView from './components/BenchmarkView.svelte';

  let isSfx = $derived(app.isSfx);
  let view = $derived(isSfx ? 'decompress' : app.currentView);

  $effect(() => {
    document.documentElement.setAttribute('data-theme', theme.current);
  });

  onMount(() => {
    const cleanups: (() => void)[] = [theme.init(), initShortcuts()];
    let disposed = false;

    /**
     * 异步注册的监听器要考虑「还没注册完组件就卸了」。
     *
     * 旧实现把 unlisten 赋值写在 `.then()` 里，销毁函数先跑一步就读到 undefined，
     * 监听器从此留在后台无人回收。
     */
    function track(fn: () => void): void {
      if (disposed) fn();
      else cleanups.push(fn);
    }

    void initProgressListener().then(track);
    void initDragDrop().then(track);
    void app.initSfx();

    return () => {
      disposed = true;
      for (const fn of cleanups.splice(0)) fn();
    };
  });
</script>

<div class="relative h-full w-full overflow-hidden bg-canvas">
  <!-- 背景极光：三团低饱和色斑撑起整体氛围，不参与命中测试。 -->
  <div
    class="aurora -top-48 -left-40 h-[34rem] w-[34rem]"
    style="background: var(--zarc-aurora-1)"
  ></div>
  <div
    class="aurora -right-52 top-1/3 h-[30rem] w-[30rem]"
    style="background: var(--zarc-aurora-2)"
  ></div>
  <div
    class="aurora bottom-[-14rem] left-1/3 h-[26rem] w-[26rem]"
    style="background: var(--zarc-aurora-3)"
  ></div>

  <div class="relative z-10 flex h-full">
    {#if !isSfx}
      <Sidebar />
    {/if}

    <main class="flex min-w-0 flex-1 flex-col gap-4 overflow-y-auto px-5 py-4">
      <TopBar />

      {#if !isSfx}
        <div class="mx-auto w-full max-w-3xl min-[1180px]:hidden">
          <TaskStrip />
        </div>
      {/if}

      <div class="mx-auto w-full max-w-3xl flex-1 pb-6">
        <!-- key 让切换视图时有一次轻微的横向位移，形成方向感。 -->
        {#key view}
          <div in:fly={{ x: 12, duration: 220, opacity: 0 }}>
            {#if view === 'decompress'}
              <DecompressView />
            {:else if view === 'compress'}
              <CompressView />
            {:else}
              <BenchmarkView />
            {/if}
          </div>
        {/key}
      </div>
    </main>

    {#if !isSfx}
      <TaskHub />
    {/if}
  </div>

  <MasterDropZone />
  <ShortcutsDialog />
  <ToastHost />
</div>

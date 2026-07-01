<script lang="ts">
  import { onMount } from 'svelte';
  import { theme } from './stores/theme.svelte';
  import { app } from './stores/app.svelte';
  import { initProgressListener } from './lib/progress';
  import { initDragDrop } from './lib/dragdrop';
  import Sidebar from './components/Sidebar.svelte';
  import TopBar from './components/TopBar.svelte';
  import TaskHub from './components/TaskHub.svelte';
  import MasterDropZone from './components/MasterDropZone.svelte';
  import CompressView from './components/CompressView.svelte';
  import DecompressView from './components/DecompressView.svelte';
  import BenchmarkView from './components/BenchmarkView.svelte';

  let isSfx = $derived(app.isSfx);
  let view = $derived(app.currentView);

  // Sync data-theme attribute whenever theme changes.
  $effect(() => {
    document.documentElement.setAttribute('data-theme', theme.current);
  });

  onMount(() => {
    theme.init();
    let unlistenProgress: (() => void) | undefined;
    let unlistenDrag: (() => void) | undefined;

    void initProgressListener().then((fn) => {
      unlistenProgress = fn;
    });
    void initDragDrop().then((fn) => {
      unlistenDrag = fn;
    });
    void app.initSfx();

    return () => {
      unlistenProgress?.();
      unlistenDrag?.();
    };
  });
</script>

<div class="relative h-full w-full overflow-hidden">
  <!-- Background orbs -->
  <div
    class="orb h-[28rem] w-[28rem] -left-40 -top-40"
    style="background: var(--bg-grad-1)"
  ></div>
  <div
    class="orb h-[32rem] w-[32rem] -right-48 bottom-[-12rem]"
    style="background: var(--bg-grad-2)"
  ></div>

  <div class="relative z-10 flex h-full">
    {#if !isSfx}
      <Sidebar />
    {/if}

    <main class="flex min-w-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
      <TopBar />
      <div class="mx-auto w-full max-w-3xl flex-1 pb-4">
        {#if isSfx || view === 'decompress'}
          <DecompressView />
        {:else if view === 'compress'}
          <CompressView />
        {:else}
          <BenchmarkView />
        {/if}
      </div>
    </main>

    {#if !isSfx}
      <TaskHub />
    {/if}
  </div>

  <MasterDropZone />
</div>

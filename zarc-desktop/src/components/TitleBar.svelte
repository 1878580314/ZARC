<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './ui/Icon.svelte';

  const win = getCurrentWindow();
  let maximized = $state(false);

  onMount(() => {
    void win.isMaximized().then((v) => (maximized = v));
    // 拖拽边缘调整尺寸也会改变 maximized，不能只靠点击后同步。
    // Resizing via the window edge also flips maximized, so listening for
    // resize events is required — toggling through the button alone is not enough.
    const unlisten = win.onResized(() => {
      void win.isMaximized().then((v) => (maximized = v));
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });
</script>

<!-- 自绘标题栏：无边框窗口下的拖拽区 + 窗口控制。data-tauri-drag-region
     交给 Tauri 处理拖动；双击沿系统惯例切换最大化。
     Custom titlebar for the undecorated window: drag region plus window
     controls. data-tauri-drag-region delegates dragging to Tauri; double
     click toggles maximize per platform convention. -->
<div class="relative z-20 flex h-9 shrink-0 select-none items-stretch">
  <div
    data-tauri-drag-region
    class="flex min-w-0 flex-1 items-center px-3.5"
    ondblclick={() => win.toggleMaximize()}
    role="presentation"
  >
    <span class="text-[0.65rem] font-semibold tracking-[0.22em] text-fg-faint">ZARC STUDIO</span>
  </div>

  <div class="flex items-stretch">
    <button
      type="button"
      onclick={() => win.minimize()}
      aria-label={t('shell.win.minimize')}
      title={t('shell.win.minimize')}
      class="flex w-11 items-center justify-center text-fg-soft transition-colors hover:bg-inset hover:text-fg"
    >
      <Icon name="winMin" size={14} stroke={1.5} />
    </button>
    <button
      type="button"
      onclick={() => win.toggleMaximize()}
      aria-label={maximized ? t('shell.win.restore') : t('shell.win.maximize')}
      title={maximized ? t('shell.win.restore') : t('shell.win.maximize')}
      class="flex w-11 items-center justify-center text-fg-soft transition-colors hover:bg-inset hover:text-fg"
    >
      <Icon name={maximized ? 'winRestore' : 'winMax'} size={13} stroke={1.5} />
    </button>
    <button
      type="button"
      onclick={() => win.close()}
      aria-label={t('shell.win.close')}
      title={t('shell.win.close')}
      class="flex w-11 items-center justify-center text-fg-soft transition-colors hover:bg-danger hover:text-white"
    >
      <Icon name="close" size={15} stroke={1.5} />
    </button>
  </div>
</div>

<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { app } from '../stores/app.svelte';
  import { SHORTCUTS } from '../lib/shortcuts';
  import Icon from './ui/Icon.svelte';

  let panel = $state<HTMLDivElement | null>(null);

  function close(): void {
    app.shortcutsOpen = false;
  }

  // 打开时把焦点交给面板，Esc 与 Tab 才有落脚点。
  $effect(() => {
    if (app.shortcutsOpen) panel?.focus();
  });
</script>

{#if app.shortcutsOpen}
  <div
    class="fixed inset-0 z-[300] flex items-center justify-center p-6"
    transition:fade={{ duration: 140 }}
  >
    <!-- 遮罩做成按钮，点击关闭的同时天然带上键盘可达性。 -->
    <button
      type="button"
      aria-label="关闭快捷键面板"
      onclick={close}
      class="absolute inset-0 bg-canvas/60 backdrop-blur-sm"
    ></button>

    <div
      bind:this={panel}
      role="dialog"
      aria-modal="true"
      aria-label="键盘快捷键"
      tabindex="-1"
      class="panel-solid relative w-full max-w-md rounded-panel p-6 shadow-[var(--shadow-lift)] outline-none"
      in:scale={{ duration: 200, start: 0.95 }}
      out:scale={{ duration: 120, start: 0.97 }}
    >
      <div class="mb-5 flex items-start justify-between gap-4">
        <div>
          <h2 class="text-base font-bold tracking-tight text-fg">键盘快捷键</h2>
          <p class="mt-0.5 text-xs text-fg-faint">在任何页面都可用</p>
        </div>
        <button
          type="button"
          onclick={close}
          aria-label="关闭"
          class="rounded-control p-1.5 text-fg-faint transition-colors hover:bg-inset hover:text-fg"
        >
          <Icon name="close" size={16} />
        </button>
      </div>

      <dl class="flex flex-col divide-y divide-line">
        {#each SHORTCUTS as item (item.description)}
          <div class="flex items-center justify-between gap-4 py-2.5">
            <dt class="text-sm text-fg-soft">{item.description}</dt>
            <dd class="flex shrink-0 items-center gap-1">
              {#each item.keys as key, i (key)}
                {#if i > 0}
                  <span class="text-[0.65rem] text-fg-faint">+</span>
                {/if}
                <kbd
                  class="mono rounded-md border border-line-strong bg-inset px-1.5 py-0.5 text-[0.7rem] font-medium text-fg-soft"
                >
                  {key}
                </kbd>
              {/each}
            </dd>
          </div>
        {/each}
      </dl>
    </div>
  </div>
{/if}

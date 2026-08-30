<script lang="ts">
  import { flip } from 'svelte/animate';
  import { fly, scale } from 'svelte/transition';
  import { toasts, type ToastLevel } from '../stores/toast.svelte';
  import { t } from '../lib/i18n/index.svelte';
  import Icon, { type IconName } from './ui/Icon.svelte';
  import CopyButton from './ui/CopyButton.svelte';

  const icons: Record<ToastLevel, IconName> = {
    info: 'info',
    success: 'checkCircle',
    warn: 'warn',
    error: 'error'
  };

  const tones: Record<ToastLevel, string> = {
    info: 'text-info',
    success: 'text-success',
    warn: 'text-warning',
    error: 'text-danger'
  };
</script>

<!-- 状态栏单行显示且会被下一条更新覆盖；toast 让并发反馈留在屏幕上。 / The status bar shows a single line and gets overwritten by the next update; toasts keep concurrent feedback on screen. -->
<div
  class="pointer-events-none fixed right-4 bottom-4 z-[200] flex w-[min(24rem,calc(100vw-2rem))] flex-col gap-2"
  aria-live="polite"
  aria-atomic="false"
>
  {#each toasts.items as toast (toast.id)}
    <div
      animate:flip={{ duration: 220 }}
      in:fly={{ x: 24, duration: 240 }}
      out:scale={{ start: 0.94, duration: 160 }}
      class="panel pointer-events-auto flex items-start gap-3 rounded-control px-3.5 py-3"
      role={toast.level === 'error' ? 'alert' : 'status'}
    >
      <span class="mt-px {tones[toast.level]}">
        <Icon name={icons[toast.level]} size={17} />
      </span>

      <div class="min-w-0 flex-1">
        <p class="text-sm leading-snug font-semibold text-fg">{toast.title}</p>
        {#if toast.detail}
          <p
            class="mt-1 max-h-28 overflow-y-auto text-xs leading-relaxed break-words whitespace-pre-line text-fg-soft"
            data-selectable
          >
            {toast.detail}
          </p>
        {/if}
      </div>

      <div class="flex shrink-0 items-center gap-0.5">
        {#if toast.detail && toast.level === 'error'}
          <CopyButton text={`${toast.title}\n${toast.detail}`} label={t('shell.toast.copyError')} />
        {/if}
        <button
          type="button"
          onclick={() => toasts.dismiss(toast.id)}
          aria-label={t('shell.toast.close')}
          class="rounded-md p-1 text-fg-faint transition-colors hover:bg-inset hover:text-fg"
        >
          <Icon name="close" size={14} />
        </button>
      </div>
    </div>
  {/each}
</div>

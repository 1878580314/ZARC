<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { dropOverlay } from '../lib/dragdrop.svelte';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './ui/Icon.svelte';
</script>

{#if dropOverlay.visible}
  <div
    class="fixed inset-0 z-[9999] flex items-center justify-center bg-canvas/55 backdrop-blur-md"
    transition:fade={{ duration: 140 }}
  >
    <div
      class="panel-solid flex flex-col items-center gap-4 rounded-[2rem] px-16 py-12 text-center shadow-[var(--shadow-lift)] outline-2 outline-offset-[-10px] outline-dashed outline-accent/55"
      in:scale={{ duration: 200, start: 0.94 }}
      out:scale={{ duration: 120, start: 0.97 }}
    >
      <span
        class="flex h-16 w-16 animate-[var(--animate-float)] items-center justify-center rounded-full bg-accent-wash text-accent"
      >
        <Icon name="dropIn" size={30} stroke={1.6} />
      </span>
      <div class="flex flex-col gap-1">
        <p class="text-lg font-bold tracking-tight text-fg">
          {#if dropOverlay.count > 1}
            {t('shell.drop.many', { count: dropOverlay.count })}
          {:else}
            {t('shell.drop.one')}
          {/if}
        </p>
        <p class="text-sm text-fg-soft">
          {t('shell.drop.routing')}
        </p>
      </div>
      {#if dropOverlay.count > 1}
        <p class="text-xs text-warning">{t('shell.drop.multiWarn')}</p>
      {/if}
    </div>
  </div>
{/if}

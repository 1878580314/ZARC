<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { api } from '../lib/api';
  import { normalizeError } from '../lib/format';
  import { t } from '../lib/i18n/index.svelte';
  import { toasts } from '../stores/toast.svelte';
  import Icon from './ui/Icon.svelte';
  import Button from './ui/Button.svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }
  let { open, onClose }: Props = $props();

  const GITHUB_AUTHOR = 'https://github.com/1878580314';
  const GITHUB_REPO = 'https://github.com/1878580314/ZARC';
  const GITHUB_RELEASES = `${GITHUB_REPO}/releases`;

  let version = $state('');
  let panelEl: HTMLElement | null = $state(null);

  $effect(() => {
    if (!open) return;
    void getVersion()
      .then((value) => (version = value))
      .catch(() => {});
    panelEl?.focus();
  });

  function openLink(url: string): void {
    void api.openUrl(url).catch((error) => {
      toasts.error(t('audit.openFailed'), normalizeError(error));
    });
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-6">
    <button
      type="button"
      class="absolute inset-0 bg-canvas/55 backdrop-blur-md"
      aria-label={t('shell.close')}
      tabindex="-1"
      onclick={onClose}
    ></button>

    <div
      bind:this={panelEl}
      role="dialog"
      aria-modal="true"
      aria-label={t('about.title')}
      tabindex="-1"
      class="panel relative flex w-full max-w-md flex-col gap-5 rounded-panel p-6 animate-[var(--animate-rise)]"
      onkeydown={(e) => e.key === 'Escape' && onClose()}
    >
      <button
        type="button"
        onclick={onClose}
        aria-label={t('shell.close')}
        class="absolute top-4 right-4 rounded-control p-1 text-fg-faint transition-colors hover:bg-inset hover:text-fg"
      >
        <Icon name="close" size={16} />
      </button>

      <div class="flex items-center gap-3.5">
        <span
          class="flex h-11 w-11 shrink-0 items-center justify-center rounded-control bg-accent text-accent-fg shadow-[var(--shadow-glow)]"
        >
          <Icon name="compress" size={22} stroke={2} />
        </span>
        <div class="flex min-w-0 flex-col">
          <span class="text-xl font-extrabold tracking-tight text-fg">ZARC</span>
          <span class="text-xs text-fg-faint">
            {version ? t('about.version', { version }) : '—'} · {t('shell.tagline')}
          </span>
        </div>
      </div>

      <Button icon="refresh" onclick={() => openLink(GITHUB_RELEASES)}>
        {t('about.checkUpdate')}
      </Button>
      <p class="-mt-3 text-[0.68rem] text-fg-faint">{t('about.updateHint')}</p>

      <div class="flex flex-col divide-y divide-line rounded-control bg-inset px-3">
        <button
          type="button"
          onclick={() => openLink(GITHUB_AUTHOR)}
          class="flex items-center gap-2.5 py-2.5 text-sm transition-colors hover:text-accent"
        >
          <Icon name="github" size={16} />
          <span class="w-20 shrink-0 text-left text-fg-faint">{t('about.author')}</span>
          <span class="min-w-0 flex-1 truncate text-left font-medium text-fg">1878580314</span>
          <Icon name="external" size={13} class="text-fg-faint" />
        </button>
        <button
          type="button"
          onclick={() => openLink(GITHUB_REPO)}
          class="flex items-center gap-2.5 py-2.5 text-sm transition-colors hover:text-accent"
        >
          <Icon name="archive" size={16} />
          <span class="w-20 shrink-0 text-left text-fg-faint">{t('about.repository')}</span>
          <span class="min-w-0 flex-1 truncate text-left font-medium text-fg">1878580314/ZARC</span>
          <Icon name="external" size={13} class="text-fg-faint" />
        </button>
      </div>

      <div class="flex flex-col gap-1.5">
        <h3 class="text-[0.68rem] font-bold tracking-[0.12em] text-fg-faint uppercase">
          {t('about.license.title')}
        </h3>
        <p class="max-h-32 overflow-y-auto text-xs leading-relaxed text-fg-soft" data-selectable>
          {t('about.license.body')}
        </p>
      </div>
    </div>
  </div>
{/if}

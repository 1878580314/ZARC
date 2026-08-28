<script lang="ts">
  import { app } from '../stores/app.svelte';
  import { theme } from '../stores/theme.svelte';
  import { task } from '../stores/task.svelte';
  import type { ViewId } from '../lib/api';
  import { t } from '../lib/i18n/index.svelte';
  import Icon, { type IconName } from './ui/Icon.svelte';
  import Spinner from './ui/Spinner.svelte';

  let navItems = $derived<
    { id: ViewId; label: string; hint: string; icon: IconName; key: string }[]
  >([
    { id: 'compress', label: t('nav.compress'), hint: t('shell.nav.compress.hint'), icon: 'compress', key: '1' },
    { id: 'decompress', label: t('nav.extract'), hint: t('shell.nav.decompress.hint'), icon: 'decompress', key: '2' },
    { id: 'benchmark', label: t('nav.benchmark'), hint: t('shell.nav.benchmark.hint'), icon: 'benchmark', key: '3' }
  ]);

  let current = $derived(app.currentView);
</script>

<aside class="flex h-full w-52 shrink-0 flex-col gap-1 px-3 py-4">
  <div class="flex items-center gap-2.5 px-2 pt-1 pb-5">
    <span
      class="flex h-9 w-9 items-center justify-center rounded-control bg-accent text-accent-fg shadow-[var(--shadow-glow)]"
    >
      <Icon name="compress" size={19} stroke={2} />
    </span>
    <span class="flex flex-col leading-none">
      <span class="text-[0.95rem] font-extrabold tracking-tight text-fg">ZARC</span>
      <span class="mt-1 text-[0.65rem] tracking-wide text-fg-faint">{t('shell.tagline')}</span>
    </span>
  </div>

  <nav class="flex flex-col gap-1" aria-label={t('shell.mainNav')}>
    {#each navItems as item (item.id)}
      {@const active = current === item.id}
      {@const running = task.activeKind === item.id}
      <button
        type="button"
        aria-current={active ? 'page' : undefined}
        onclick={() => app.setView(item.id)}
        title="{item.hint} (Ctrl+{item.key})"
        class="group relative flex items-center gap-2.5 rounded-control px-2.5 py-2 text-sm transition-colors duration-200 {active
          ? 'bg-accent-wash text-accent'
          : 'text-fg-soft hover:bg-inset hover:text-fg'}"
      >
        <!-- A slim indicator bar is subtler than a filled block and keeps the icon's own color. -->
        <span
          class="absolute top-1/2 left-0 h-5 w-[3px] -translate-y-1/2 rounded-r-full bg-accent transition-opacity duration-200 {active
            ? 'opacity-100'
            : 'opacity-0'}"
        ></span>

        {#if running}
          <Spinner size={16} class="text-accent" />
        {:else}
          <Icon name={item.icon} size={17} />
        {/if}

        <span class="flex-1 text-left font-medium">{item.label}</span>

        <kbd
          class="rounded border border-line px-1 text-[0.6rem] text-fg-faint opacity-0 transition-opacity group-hover:opacity-100"
        >
          {item.key}
        </kbd>
      </button>
    {/each}
  </nav>

  <div class="mt-auto flex flex-col gap-1 border-t border-line pt-3">
    <button
      type="button"
      onclick={() => (app.shortcutsOpen = true)}
      title={t('shell.shortcutsButton')}
      class="flex items-center gap-2.5 rounded-control px-2.5 py-2 text-sm text-fg-soft transition-colors hover:bg-inset hover:text-fg"
    >
      <Icon name="sliders" size={17} />
      <span class="flex-1 text-left font-medium">{t('shell.nav.shortcuts')}</span>
      <kbd class="rounded border border-line px-1 text-[0.6rem] text-fg-faint">/</kbd>
    </button>

    <button
      type="button"
      onclick={() => theme.toggle()}
      title={t('shell.themeShortcut')}
      class="flex items-center gap-2.5 rounded-control px-2.5 py-2 text-sm text-fg-soft transition-colors hover:bg-inset hover:text-fg"
    >
      <Icon name={theme.current === 'dark' ? 'moon' : 'sun'} size={17} />
      <span class="flex-1 text-left font-medium">
        {theme.current === 'dark' ? t('shell.theme.dark') : t('shell.theme.light')}
      </span>
      <kbd class="rounded border border-line px-1 text-[0.6rem] text-fg-faint">D</kbd>
    </button>
  </div>
</aside>

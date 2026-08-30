<script lang="ts">
  import { app, type StatusLevel } from '../stores/app.svelte';
  import { theme } from '../stores/theme.svelte';
  import { t, toggleLocale, currentLocale } from '../lib/i18n/index.svelte';
  import Icon, { type IconName } from './ui/Icon.svelte';
  import Spinner from './ui/Spinner.svelte';

  let view = $derived(app.currentView);
  let title = $derived(app.isSfx ? t('shell.sfx.modeTitle') : t(`shell.view.${view}.title`));
  let subtitle = $derived(
    app.isSfx ? t('shell.sfx.modeSubtitle') : t(`shell.view.${view}.subtitle`)
  );
  let status = $derived(app.status);

  const tone: Record<StatusLevel, string> = {
    idle: 'text-fg-faint',
    busy: 'text-accent',
    success: 'text-success',
    error: 'text-danger'
  };

  const statusIcon: Record<StatusLevel, IconName> = {
    idle: 'info',
    busy: 'info',
    success: 'checkCircle',
    error: 'error'
  };
</script>

<header class="flex items-start justify-between gap-6 px-1">
  <div class="min-w-0">
    <h1 class="text-[1.35rem] leading-tight font-extrabold tracking-tight text-fg">{title}</h1>
    <p class="mt-1 text-xs text-fg-faint">{subtitle}</p>
  </div>

  <div class="flex shrink-0 items-center gap-2">
    <div
      class="panel flex max-w-[22rem] items-center gap-2 rounded-pill px-3 py-1.5 text-xs font-medium {tone[
        status.level
      ]}"
      role="status"
      aria-live="polite"
    >
      {#if status.level === 'busy'}
        <Spinner size={12} />
      {:else}
        <Icon name={statusIcon[status.level]} size={13} />
      {/if}
      <span class="truncate" title={status.message}>{status.message}</span>
    </div>

    <!-- 语言切换与主题切换同处一行，角落保持单一控制行。 / Language toggle lives next to the theme toggle so the corner stays one control row. -->
    <button
      type="button"
      onclick={toggleLocale}
      aria-label={t('shell.localeToggle')}
      title={t('shell.localeToggle')}
      class="panel flex h-8 min-w-8 items-center justify-center rounded-pill px-2.5 text-[0.7rem] font-semibold tracking-wide text-fg-soft transition-colors hover:text-fg"
    >
      {currentLocale() === 'zh' ? 'EN' : '中文'}
    </button>

    <!-- SFX 模式没有侧边栏，主题切换也需要此处的入口。 / SFX mode has no sidebar, so the theme toggle needs an entry point here too. -->
    {#if app.isSfx}
      <button
        type="button"
        onclick={() => theme.toggle()}
        aria-label={t('shell.themeToggle')}
        title={t('shell.themeToggle')}
        class="panel flex h-8 w-8 items-center justify-center rounded-pill text-fg-soft transition-colors hover:text-fg"
      >
        <Icon name={theme.current === 'dark' ? 'moon' : 'sun'} size={15} />
      </button>
    {/if}
  </div>
</header>

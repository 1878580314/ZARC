<script lang="ts">
  import { app } from '../stores/app.svelte';
  import { formatBytes, sidecarName } from '../lib/format';
  import { t } from '../lib/i18n/index.svelte';
  import Icon from './ui/Icon.svelte';
  import Tag from './ui/Tag.svelte';

  let info = $derived(app.sfxInfo);
  let payloadMissing = $derived(info !== null && !info.payloadReady);
  let expectedSidecar = $derived(
    info ? sidecarName(info.hostPath) : ''
  );
</script>

{#if info}
  <div
    class="panel flex items-start gap-3.5 rounded-panel p-4 animate-[var(--animate-rise)] {payloadMissing
      ? 'border-warning/50'
      : 'border-accent/30 bg-accent-wash'}"
  >
    <span
      class="flex h-10 w-10 shrink-0 items-center justify-center rounded-control shadow-[var(--shadow-glow)] {payloadMissing
        ? 'bg-warning text-white'
        : 'bg-accent text-accent-fg'}"
    >
      <Icon name={payloadMissing ? 'error' : 'archive'} size={19} />
    </span>

    <div class="flex min-w-0 flex-col gap-1.5">
      <p class="text-sm font-bold text-fg">{t('shell.sfx.title')}</p>
      <p class="text-xs leading-relaxed text-fg-soft">
        {t('shell.sfx.body')}
      </p>
      {#if payloadMissing}
        <p class="text-xs font-semibold leading-relaxed text-warning">
          {t('shell.sfx.payloadMissing', { name: expectedSidecar })}
        </p>
      {/if}
      <div class="mt-0.5 flex flex-wrap items-center gap-1.5">
        <Tag tone="accent">{info.archiveKind}</Tag>
        <Tag tone="neutral">{formatBytes(info.payloadBytes)}</Tag>
        <Tag tone="neutral" icon="folder">{info.defaultExtractName}</Tag>
        {#if payloadMissing}
          <Tag tone="warning" icon="error">{t('shell.sfx.payloadMissingTag')}</Tag>
        {/if}
        {#if info.encrypted}
          <Tag tone="warning" icon="shield">{t('shell.sfx.encrypted')}</Tag>
        {/if}
      </div>
    </div>
  </div>
{/if}

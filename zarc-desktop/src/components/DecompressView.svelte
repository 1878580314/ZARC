<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { app } from '../stores/app.svelte';
  import { task } from '../stores/task.svelte';
  import { progress } from '../stores/progress.svelte';
  import { toasts } from '../stores/toast.svelte';
  import { registerPrimaryAction } from '../lib/shortcuts';
  import { api, type ArchiveContentReport, type OperationReport } from '../lib/api';
  import { emptyToNull, formatBytes, pathBaseName } from '../lib/format';
  import { t } from '../lib/i18n/index.svelte';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';
  import Field from './ui/Field.svelte';
  import PathInput from './ui/PathInput.svelte';
  import PasswordInput from './ui/PasswordInput.svelte';
  import Tag from './ui/Tag.svelte';
  import SfxBanner from './SfxBanner.svelte';
  import ArchiveBrowser from './ArchiveBrowser.svelte';
  import ResultCard from './ResultCard.svelte';
  import ProgressBar from './ProgressBar.svelte';

  let password = $state('');
  let output = $state('');
  let report = $state<OperationReport | null>(null);
  let browserReport = $state<ArchiveContentReport | null>(null);
  let touched = $state(false);
  /** Preview and extraction share the decompress slot; this flag tells "reading the list" apart from a real extraction. */
  let previewing = $state(false);

  let isSfx = $derived(app.isSfx);
  let source = $derived(isSfx ? (app.sfxInfo?.hostPath ?? '') : app.decompressSource);
  let busy = $derived(task.busy);
  let running = $derived(task.activeKind === 'decompress');

  let sourceError = $derived(touched && !source ? t('decompress.error.noSource') : undefined);
  let outputError = $derived(
    touched && isSfx && !output.trim() ? t('decompress.error.outputRequired') : undefined
  );

  let outputPlaceholder = $derived(
    isSfx && app.sfxInfo
      ? t('decompress.outputPlaceholder.sfx', { name: app.sfxInfo.defaultExtractName })
      : t('decompress.outputPlaceholder.default')
  );

  $effect(() => registerPrimaryAction('decompress', submit));

  async function pickSource(): Promise<void> {
    const selected = await open({
      title: t('decompress.dialog.pickArchive'),
      multiple: false,
      directory: false,
      filters: [
        { name: t('decompress.filter.zarc'), extensions: ['zst', 'enc', 'exe'] },
        { name: t('decompress.filter.all'), extensions: ['*'] }
      ]
    });
    if (typeof selected === 'string') {
      app.setDecompressSource(selected);
      browserReport = null;
    }
  }

  async function pickOutput(): Promise<void> {
    const selected = await open({ title: t('decompress.dialog.pickOutput'), multiple: false, directory: true });
    if (typeof selected === 'string') output = selected;
  }

  async function preview(): Promise<void> {
    touched = true;
    if (!source) {
      app.setStatus(t('decompress.error.archiveRequired'), 'error');
      return;
    }
    previewing = true;
    try {
      const ok = await task.run('decompress', t('decompress.running.list'), async () => {
        const listed = await api.listContent({
          archivePath: source,
          password: emptyToNull(password)
        });
        browserReport = listed;
        app.setStatus(t('decompress.status.listed', { count: listed.totalFiles }), 'success');
      });
      // Preview only reads metadata; there's no point leaving a 100% decompress card in the task hub.
      if (ok) progress.hide('decompress');
    } finally {
      previewing = false;
    }
  }

  async function submit(): Promise<void> {
    touched = true;
    if (!isSfx && !source) {
      app.setStatus(t('decompress.error.archiveRequired'), 'error');
      toasts.warn(t('toast.noArchive'), t('decompress.hint.noArchive'));
      return;
    }
    if (isSfx && !output.trim()) {
      app.setStatus(t('decompress.error.outputFolderRequired'), 'error');
      toasts.warn(t('decompress.toast.noOutput'), t('decompress.hint.noOutput'));
      return;
    }
    if (isSfx && app.sfxInfo && !app.sfxInfo.payloadReady) {
      const name = `${pathBaseName(app.sfxInfo.hostPath)}.payload`;
      app.setStatus(t('shell.sfx.payloadMissingTag'), 'error');
      toasts.warn(t('shell.sfx.payloadMissingTag'), t('shell.sfx.payloadMissing', { name }));
      return;
    }

    const ok = await task.run('decompress', t('decompress.running', { name: pathBaseName(source) }), async () => {
      report = isSfx
        ? await api.extractEmbedded({
            outputPath: emptyToNull(output),
            password: emptyToNull(password)
          })
        : await api.decompress({
            archivePath: source,
            outputPath: emptyToNull(output),
            password: emptyToNull(password)
          });
      app.setStatus(t('decompress.status.done', { path: report.outputPath }), 'success');
    });

    if (ok && report) {
      toasts.success(t('toast.extractionComplete'), report.outputPath);
    }
  }
</script>

<div class="flex flex-col gap-4 animate-[var(--animate-rise)]">
  <SfxBanner />

  <Card
    title={t('decompress.settingsCard.title')}
    subtitle={isSfx ? t('decompress.settingsCard.subtitle.sfx') : t('decompress.settingsCard.subtitle.default')}
    icon="decompress"
  >
    {#snippet actions()}
      {#if app.sfxInfo?.encrypted || (!isSfx && source.toLowerCase().endsWith('.enc'))}
        <Tag tone="warning" icon="shield">{t('decompress.tag.passwordRequired')}</Tag>
      {/if}
    {/snippet}

    <div class="flex flex-col gap-4">
      <Field
        label={t('decompress.archiveSource')}
        error={sourceError}
        hint={isSfx ? t('decompress.archiveSourceHint.sfx') : undefined}
      >
        {#snippet aside()}
          {#if isSfx && app.sfxInfo}
            {formatBytes(app.sfxInfo.payloadBytes)}
          {/if}
        {/snippet}
        <PathInput
          value={source}
          onCommit={(next) => !isSfx && app.setDecompressSource(next)}
          icon="archive"
          readonly={isSfx}
          invalid={Boolean(sourceError)}
          placeholder={t('decompress.sourcePlaceholder')}
        >
          {#snippet actions()}
            <Button variant="ghost" size="sm" icon="folder" onclick={pickSource} disabled={isSfx}>
              {t('browse')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              icon="search"
              onclick={preview}
              loading={previewing}
              disabled={isSfx || (busy && !previewing)}
            >
              {t('decompress.preview')}
            </Button>
          {/snippet}
        </PathInput>
      </Field>

      <Field label={t('decompress.outputFolder')} error={outputError}>
        <PathInput
          bind:value={output}
          icon="folder"
          invalid={Boolean(outputError)}
          placeholder={outputPlaceholder}
        >
          {#snippet actions()}
            <Button variant="ghost" size="sm" icon="folder" onclick={pickOutput}>{t('browse')}</Button>
          {/snippet}
        </PathInput>
      </Field>

      <Field label={t('decompress.password')} hint={t('decompress.passwordHint')}>
        <PasswordInput bind:value={password} placeholder={t('decompress.passwordPlaceholder')} />
      </Field>

      <div class="flex items-center gap-3 border-t border-line pt-4">
        <Button
          icon="play"
          loading={running && !previewing}
          disabled={(busy && !running) || (isSfx && app.sfxInfo !== null && !app.sfxInfo.payloadReady)}
          onclick={submit}
        >
          {t('decompress.submit')}
        </Button>
        {#if running && !previewing}
          <Button
            variant="danger"
            icon="stop"
            disabled={task.aborting}
            onclick={() => task.requestAbort()}
          >
            {task.aborting ? t('task.stopping') : t('task.stop')}
          </Button>
        {/if}
        <span class="ml-auto text-[0.7rem] text-fg-faint">Ctrl + Enter</span>
      </div>
    </div>
  </Card>

  <!-- Self-extracting mode has no sidebar task hub, so progress is shown inline here. -->
  {#if isSfx && progress.decompress.visible}
    <div class="panel rounded-panel px-5 py-4">
      <ProgressBar progress={progress.decompress} label={t('decompress.progressLabel')} />
    </div>
  {/if}

  {#if browserReport}
    <ArchiveBrowser report={browserReport} onClose={() => (browserReport = null)} />
  {/if}

  {#if report}
    <ResultCard title={t('toast.extractionComplete')} {report} onDismiss={() => (report = null)} />
  {/if}
</div>

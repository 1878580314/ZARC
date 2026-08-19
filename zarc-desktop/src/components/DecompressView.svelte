<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { app } from '../stores/app.svelte';
  import { task } from '../stores/task.svelte';
  import { progress } from '../stores/progress.svelte';
  import { toasts } from '../stores/toast.svelte';
  import { registerPrimaryAction } from '../lib/shortcuts';
  import { api, type ArchiveContentReport, type OperationReport } from '../lib/api';
  import { emptyToNull, formatBytes, pathBaseName } from '../lib/format';
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
  /** 预览与解压共用 decompress 槽位，用它区分「读列表」和「真解压」。 */
  let previewing = $state(false);

  let isSfx = $derived(app.isSfx);
  let source = $derived(isSfx ? (app.sfxInfo?.hostPath ?? '') : app.decompressSource);
  let busy = $derived(task.busy);
  let running = $derived(task.activeKind === 'decompress');

  let sourceError = $derived(touched && !source ? '请先选择要解压的归档文件。' : undefined);
  let outputError = $derived(
    touched && isSfx && !output.trim() ? '自解压模式必须指定输出目录。' : undefined
  );

  let outputPlaceholder = $derived(
    isSfx && app.sfxInfo
      ? `选择输出目录，将生成 ${app.sfxInfo.defaultExtractName}`
      : '留空则解压到归档所在目录'
  );

  $effect(() => registerPrimaryAction('decompress', submit));

  async function pickSource(): Promise<void> {
    const selected = await open({
      title: '选择归档文件',
      multiple: false,
      directory: false,
      filters: [
        { name: 'ZARC 归档', extensions: ['zst', 'enc', 'exe'] },
        { name: '全部文件', extensions: ['*'] }
      ]
    });
    if (typeof selected === 'string') {
      app.setDecompressSource(selected);
      browserReport = null;
    }
  }

  async function pickOutput(): Promise<void> {
    const selected = await open({ title: '选择解压输出目录', multiple: false, directory: true });
    if (typeof selected === 'string') output = selected;
  }

  async function preview(): Promise<void> {
    touched = true;
    if (!source) {
      app.setStatus('请先选择归档文件。', 'error');
      return;
    }
    previewing = true;
    try {
      const ok = await task.run('decompress', '正在读取归档列表...', async () => {
        const listed = await api.listContent({
          archivePath: source,
          password: emptyToNull(password)
        });
        browserReport = listed;
        app.setStatus(`已列出 ${listed.totalFiles} 个条目。`, 'success');
      });
      // 预览只读元数据，没必要在任务中心留下一张 100% 的解压卡片。
      if (ok) progress.hide('decompress');
    } finally {
      previewing = false;
    }
  }

  async function submit(): Promise<void> {
    touched = true;
    if (!isSfx && !source) {
      app.setStatus('请先选择归档文件。', 'error');
      toasts.warn('还没有选择归档', '点击「选择」，也可以直接把归档拖进窗口。');
      return;
    }
    if (isSfx && !output.trim()) {
      app.setStatus('请选择解压输出目录。', 'error');
      toasts.warn('还没有选择输出目录', '自解压包不知道该把文件放到哪里。');
      return;
    }

    const ok = await task.run('decompress', `正在解压 ${pathBaseName(source)}...`, async () => {
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
      app.setStatus(`解压完成: ${report.outputPath}`, 'success');
    });

    if (ok && report) {
      toasts.success('解压完成', report.outputPath);
    }
  }
</script>

<div class="flex flex-col gap-4 animate-[var(--animate-rise)]">
  <SfxBanner />

  <Card
    title="解压设置"
    subtitle={isSfx ? '归档已内嵌，只需决定放到哪里' : '选择归档并指定输出位置'}
    icon="decompress"
  >
    {#snippet actions()}
      {#if app.sfxInfo?.encrypted || (!isSfx && source.toLowerCase().endsWith('.enc'))}
        <Tag tone="warning" icon="shield">需要密码</Tag>
      {/if}
    {/snippet}

    <div class="flex flex-col gap-4">
      <Field
        label="归档源"
        error={sourceError}
        hint={isSfx ? '自解压模式下由程序自身提供，不可更改。' : undefined}
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
          placeholder="选择 .zst / .enc / .001 归档"
        >
          {#snippet actions()}
            <Button variant="ghost" size="sm" icon="folder" onclick={pickSource} disabled={isSfx}>
              选择
            </Button>
            <Button
              variant="ghost"
              size="sm"
              icon="search"
              onclick={preview}
              loading={previewing}
              disabled={isSfx || (busy && !previewing)}
            >
              预览
            </Button>
          {/snippet}
        </PathInput>
      </Field>

      <Field label="输出目录" error={outputError}>
        <PathInput
          bind:value={output}
          icon="folder"
          invalid={Boolean(outputError)}
          placeholder={outputPlaceholder}
        >
          {#snippet actions()}
            <Button variant="ghost" size="sm" icon="folder" onclick={pickOutput}>选择</Button>
          {/snippet}
        </PathInput>
      </Field>

      <Field label="密码" hint="非加密归档留空即可。">
        <PasswordInput bind:value={password} placeholder="加密归档请输入密码" />
      </Field>

      <div class="flex items-center gap-3 border-t border-line pt-4">
        <Button
          icon="play"
          loading={running && !previewing}
          disabled={busy && !running}
          onclick={submit}
        >
          开始解压
        </Button>
        {#if running && !previewing}
          <Button
            variant="danger"
            icon="stop"
            disabled={task.aborting}
            onclick={() => task.requestAbort()}
          >
            {task.aborting ? '正在停止' : '停止'}
          </Button>
        {/if}
        <span class="ml-auto text-[0.7rem] text-fg-faint">Ctrl + Enter</span>
      </div>
    </div>
  </Card>

  <!-- 自解压模式没有侧栏任务中心，进度只能在这里就地显示。 -->
  {#if isSfx && progress.decompress.visible}
    <div class="panel rounded-panel px-5 py-4">
      <ProgressBar progress={progress.decompress} label="正在解压..." />
    </div>
  {/if}

  {#if browserReport}
    <ArchiveBrowser report={browserReport} onClose={() => (browserReport = null)} />
  {/if}

  {#if report}
    <ResultCard title="解压完成" {report} onDismiss={() => (report = null)} />
  {/if}
</div>

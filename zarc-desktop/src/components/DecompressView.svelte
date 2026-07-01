<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { app } from '../stores/app.svelte';
  import { task } from '../stores/task.svelte';
  import { api } from '../lib/api';
  import type { ArchiveContentReport } from '../lib/api';
  import { formatOperation, emptyToNull } from '../lib/format';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';
  import FileInput from './ui/FileInput.svelte';
  import TextInput from './ui/TextInput.svelte';
  import SfxBanner from './SfxBanner.svelte';
  import ArchiveBrowser from './ArchiveBrowser.svelte';

  let password = $state('');
  let output = $state('');
  let result = $state('');
  let browserReport = $state<ArchiveContentReport | null>(null);

  let isSfx = $derived(app.isSfx);
  let source = $derived(isSfx ? app.sfxInfo?.hostPath ?? '' : app.decompressSource);
  let busy = $derived(task.busy);
  let isAbort = $derived(task.activeKind === 'decompress' && task.aborting);
  let outputPlaceholder = $derived(
    isSfx && app.sfxInfo
      ? `请选择输出目录，将生成 ${app.sfxInfo.defaultExtractName}`
      : '选择解压输出目录'
  );

  async function pickSource() {
    const selected = await open({
      title: '选择归档文件',
      multiple: false,
      directory: false,
      filters: [{ name: 'Archive', extensions: ['zst', 'enc'] }]
    });
    if (typeof selected === 'string') app.setDecompressSource(selected);
  }

  async function pickOutput() {
    const selected = await open({ title: '选择解压输出目录', multiple: false, directory: true });
    if (typeof selected === 'string') output = selected;
  }

  async function preview() {
    if (!source) {
      app.setStatus('请先选择归档文件。', 'error');
      return;
    }
    await task.run('decompress', '正在读取归档列表...', async () => {
      const report = await api.listContent({
        archivePath: source,
        password: emptyToNull(password)
      });
      browserReport = report;
      app.setStatus('归档预览已加载。', 'success');
    });
  }

  async function submit() {
    if (!isSfx && !source) {
      app.setStatus('请先选择归档文件。', 'error');
      return;
    }
    if (isSfx && !output.trim()) {
      app.setStatus('请选择解压输出目录。', 'error');
      return;
    }

    await task.run('decompress', '正在解压，请稍候...', async () => {
      const report = isSfx
        ? await api.extractEmbedded({
            outputPath: emptyToNull(output),
            password: emptyToNull(password)
          })
        : await api.decompress({
            archivePath: source,
            outputPath: emptyToNull(output),
            password: emptyToNull(password)
          });
      result = formatOperation(report);
      app.setStatus(`解压完成: ${report.outputPath}`, 'success');
    });
  }

  function abort() {
    task.requestAbort();
  }
</script>

<div class="flex flex-col gap-5 animate-[var(--animate-fade-in-scale)]">
  {#if isSfx}
    <SfxBanner />
  {/if}

  <Card>
    <div class="mb-5 flex items-center justify-between">
      <h3 class="text-base font-bold text-primary">解压配置</h3>
    </div>

    <div class="flex flex-col gap-4">
      <div class="flex flex-col gap-2">
        <span class="text-xs font-medium text-secondary">归档源</span>
        <div class="flex gap-2">
          <FileInput value={source} placeholder="选择归档文件" disabled={isSfx} class="flex-1" />
          <Button variant="ghost" onclick={pickSource} disabled={isSfx}>选择</Button>
          <Button variant="ghost" onclick={preview} disabled={isSfx}>预览</Button>
        </div>
      </div>

      <div class="flex flex-col gap-2">
        <span class="text-xs font-medium text-secondary">输出目录</span>
        <div class="flex gap-2">
          <FileInput bind:value={output} placeholder={outputPlaceholder} class="flex-1" />
          <Button variant="ghost" onclick={pickOutput}>选择</Button>
        </div>
      </div>

      <div class="flex flex-col gap-2">
        <span class="text-xs font-medium text-secondary">密码（可选）</span>
        <TextInput bind:value={password} type="password" placeholder="加密归档请输入密码" />
      </div>

      <div class="flex gap-3 pt-2">
        <Button onclick={submit} disabled={busy}>开始解压</Button>
        {#if task.activeKind === 'decompress'}
          <Button variant="danger" onclick={abort} disabled={isAbort}>
            {isAbort ? '正在停止...' : '停止'}
          </Button>
        {/if}
      </div>
    </div>
  </Card>

  {#if browserReport}
    <ArchiveBrowser report={browserReport} onClose={() => (browserReport = null)} />
  {/if}

  {#if result}
    <Card>
      <h3 class="mb-3 text-base font-bold text-primary">解压结果</h3>
      <pre class="overflow-x-auto whitespace-pre-wrap rounded-2xl bg-[var(--surface-hover)] p-4 text-xs leading-relaxed text-secondary">{result}</pre>
    </Card>
  {/if}
</div>

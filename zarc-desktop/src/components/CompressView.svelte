<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { app } from '../stores/app.svelte';
  import { task } from '../stores/task.svelte';
  import { api } from '../lib/api';
  import type { OutputKind } from '../lib/api';
  import { formatOperation, toInt, emptyToNull } from '../lib/format';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';
  import FileInput from './ui/FileInput.svelte';
  import TextInput from './ui/TextInput.svelte';
  import Select from './ui/Select.svelte';
  import Slider from './ui/Slider.svelte';
  import NumberInput from './ui/NumberInput.svelte';
  import Toggle from './ui/Toggle.svelte';
  import Tag from './ui/Tag.svelte';

  let source = $state('');
  let kind = $state<'文件' | '目录'>('文件');
  let output = $state('');
  let outputKind = $state<OutputKind>('archive');
  let level = $state(8);
  let splitSize = $state(0);
  let includeRootDir = $state(true);
  let encrypt = $state(false);
  let password = $state('');
  let enableLogging = $state(false);
  let deleteSourceAfter = $state(false);
  let result = $state('');

  // Sync shared source state (drag-drop populates app store).
  $effect(() => {
    if (app.compressSource) {
      source = app.compressSource;
      kind = app.compressKind;
    }
  });

  let isSfx = $derived(outputKind === 'sfxExe');
  let splitDisabled = $derived(isSfx);
  let modeHint = $derived(
    isSfx
      ? 'Windows 自解压 EXE 会生成单个 .exe 文件，双击后进入解压模式；当前版本不支持分卷。'
      : '普通归档支持当前 .zst/.tar.zst/.enc 输出格式，并保留分卷能力。'
  );
  let busy = $derived(task.busy);
  let isAbort = $derived(task.activeKind === 'compress' && task.aborting);

  const outputOptions: { value: OutputKind; label: string }[] = [
    { value: 'archive', label: '普通归档 (.zst/.tar.zst)' },
    { value: 'sfxExe', label: '自解压 EXE (.exe)' }
  ];

  // Keep split size cleared for SFX.
  $effect(() => {
    if (isSfx) splitSize = 0;
  });

  async function pickFile() {
    kind = '文件';
    const selected = await open({ title: '选择待压缩文件', multiple: false, directory: false });
    if (typeof selected === 'string') source = selected;
  }

  async function pickDirectory() {
    kind = '目录';
    const selected = await open({ title: '选择待压缩目录', multiple: false, directory: true });
    if (typeof selected === 'string') source = selected;
  }

  async function pickOutput() {
    const selected = await save({
      title: '压缩输出路径',
      filters:
        outputKind === 'sfxExe'
          ? [{ name: 'Windows Self Extracting EXE', extensions: ['exe'] }]
          : [{ name: 'Archive', extensions: ['zst', 'enc'] }]
    });
    if (typeof selected === 'string') output = selected;
  }

  async function submit() {
    if (!source) {
      app.setStatus('请先选择压缩源路径。', 'error');
      return;
    }
    const pw = encrypt ? emptyToNull(password) : null;
    if (encrypt && !pw) {
      app.setStatus('启用加密时必须输入密码。', 'error');
      return;
    }
    const split = isSfx ? 0 : toInt(String(splitSize), 0);

    await task.run('compress', '正在压缩，请稍候...', async () => {
      const report = await api.compress({
        sourcePath: source,
        outputPath: emptyToNull(output),
        outputKind,
        level,
        includeRootDir,
        password: pw,
        splitSizeMib: split > 0 ? split : null,
        enableLogging,
        deleteSourceAfter
      });
      result = formatOperation(report);
      const sfxHint = report.sidecarPath
        ? '（同目录 .payload 为数据文件，分发请一同携带）'
        : '';
      app.setStatus(`压缩完成: ${report.outputPath}${sfxHint}`, 'success');
    });
  }

  function abort() {
    task.requestAbort();
  }
</script>

<div class="flex flex-col gap-5 animate-[var(--animate-fade-in-scale)]">
  <Card>
    <div class="mb-5 flex items-center justify-between">
      <h3 class="text-base font-bold text-primary">压缩配置</h3>
      <Tag>{kind}</Tag>
    </div>

    <div class="flex flex-col gap-4">
      <div class="flex flex-col gap-2">
        <span class="text-xs font-medium text-secondary">源路径</span>
        <div class="flex gap-2">
          <FileInput bind:value={source} placeholder="选择文件或目录" class="flex-1" />
          <Button variant="ghost" onclick={pickFile}>文件</Button>
          <Button variant="ghost" onclick={pickDirectory}>目录</Button>
        </div>
      </div>

      <div class="flex flex-col gap-2">
        <span class="text-xs font-medium text-secondary">输出路径</span>
        <div class="flex gap-2">
          <FileInput bind:value={output} placeholder="留空则自动生成" class="flex-1" />
          <Button variant="ghost" onclick={pickOutput}>选择</Button>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div class="flex flex-col gap-2">
          <span class="text-xs font-medium text-secondary">输出类型</span>
          <Select bind:value={outputKind} options={outputOptions} />
        </div>
        <div class="flex flex-col gap-2">
          <span class="text-xs font-medium text-secondary">分卷大小</span>
          <NumberInput bind:value={splitSize} suffix="MiB" disabled={splitDisabled} min={0} />
        </div>
      </div>

      <Slider bind:value={level} min={1} max={22} label="压缩等级" />

      <p class="text-xs text-muted">{modeHint}</p>

      <div class="grid grid-cols-2 gap-2 rounded-2xl bg-[var(--surface-hover)] p-2">
        <Toggle bind:checked={includeRootDir} label="包含根目录" />
        <Toggle bind:checked={encrypt} label="启用加密" />
        <Toggle bind:checked={enableLogging} label="启用日志" />
        <Toggle bind:checked={deleteSourceAfter} label="完成后删除源" />
      </div>

      {#if encrypt}
        <div class="flex flex-col gap-2">
          <span class="text-xs font-medium text-secondary">密码</span>
          <TextInput bind:value={password} type="password" placeholder="输入加密密码" />
        </div>
      {/if}

      <div class="flex gap-3 pt-2">
        <Button onclick={submit} disabled={busy}>开始压缩</Button>
        {#if task.activeKind === 'compress'}
          <Button variant="danger" onclick={abort} disabled={isAbort}>
            {isAbort ? '正在停止...' : '停止'}
          </Button>
        {/if}
      </div>
    </div>
  </Card>

  {#if result}
    <Card>
      <h3 class="mb-3 text-base font-bold text-primary">压缩结果</h3>
      <pre class="overflow-x-auto whitespace-pre-wrap rounded-2xl bg-[var(--surface-hover)] p-4 text-xs leading-relaxed text-secondary">{result}</pre>
    </Card>
  {/if}
</div>

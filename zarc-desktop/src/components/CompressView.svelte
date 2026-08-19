<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { app } from '../stores/app.svelte';
  import { task } from '../stores/task.svelte';
  import { toasts } from '../stores/toast.svelte';
  import { registerPrimaryAction } from '../lib/shortcuts';
  import { api, type OperationReport, type OutputKind } from '../lib/api';
  import { emptyToNull, formatBytes, formatCount, pathBaseName } from '../lib/format';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';
  import Field from './ui/Field.svelte';
  import PathInput from './ui/PathInput.svelte';
  import PasswordInput from './ui/PasswordInput.svelte';
  import Segmented, { type SegmentOption } from './ui/Segmented.svelte';
  import Slider from './ui/Slider.svelte';
  import NumberInput from './ui/NumberInput.svelte';
  import Toggle from './ui/Toggle.svelte';
  import Tag from './ui/Tag.svelte';
  import Icon from './ui/Icon.svelte';
  import ResultCard from './ResultCard.svelte';

  let output = $state('');
  let outputKind = $state<OutputKind>('archive');
  let splitSize = $state(0);
  /** 0 表示交给后端按核心数决定。 */
  let threads = $state(0);
  let includeRootDir = $state(true);
  let encrypt = $state(false);
  let password = $state('');
  let enableLogging = $state(false);
  let deleteSourceAfter = $state(false);
  let advanced = $state(false);
  let report = $state<OperationReport | null>(null);
  let touched = $state(false);

  // 数据源只有 app store 一份。旧实现在这里另存一份 local state 并用 $effect
  // 单向同步，于是文件对话框选出的路径写不回 store，拖放又会把它覆盖掉。
  let source = $derived(app.compressSource);
  let kind = $derived(app.compressKind);
  let info = $derived(app.compressInfo);
  // 等级同样放在 store 里，性能测试页的「采用推荐等级」才能直接落到这里。
  let level = $derived(app.compressLevel);

  let isSfx = $derived(outputKind === 'sfxExe');
  let busy = $derived(task.busy);
  let running = $derived(task.activeKind === 'compress');

  let sourceError = $derived(
    !touched ? undefined : !source ? '请先选择要压缩的文件或目录。' : undefined
  );
  let passwordError = $derived(
    touched && encrypt && !emptyToNull(password) ? '启用加密后必须设置密码。' : undefined
  );

  const outputOptions: SegmentOption<OutputKind>[] = [
    {
      value: 'archive',
      label: '普通归档',
      icon: 'archive',
      hint: '输出 .zst / .tar.zst / .enc，支持分卷'
    },
    {
      value: 'sfxExe',
      label: '自解压 EXE',
      icon: 'app',
      hint: '输出单个 .exe，双击即可解压；不支持分卷'
    }
  ];

  const levelMarks = [
    { at: 1, label: '最快' },
    { at: 8, label: '均衡' },
    { at: 15, label: '高压缩' },
    { at: 22, label: '极限' }
  ];

  let levelHint = $derived(
    level <= 4
      ? '低等级速度优先，适合临时打包或超大目录。'
      : level <= 12
        ? '中等区间是速度与体积的甜点区，日常首选。'
        : level <= 19
          ? '高等级明显更慢，收益开始递减。'
          : '20 以上会启用长距离匹配，内存占用大幅上升。'
  );

  // 自解压包必须是单文件，分卷在这里没有意义。
  $effect(() => {
    if (isSfx) splitSize = 0;
  });

  // 让 Ctrl+Enter 在压缩页触发这里的主操作。
  $effect(() => registerPrimaryAction('compress', submit));

  async function pickFile(): Promise<void> {
    const selected = await open({ title: '选择待压缩文件', multiple: false, directory: false });
    if (typeof selected === 'string') app.setCompressSource(selected, '文件');
  }

  async function pickDirectory(): Promise<void> {
    const selected = await open({ title: '选择待压缩目录', multiple: false, directory: true });
    if (typeof selected === 'string') app.setCompressSource(selected, '目录');
  }

  async function pickOutput(): Promise<void> {
    const selected = await save({
      title: '压缩输出路径',
      filters: isSfx
        ? [{ name: 'Windows 自解压程序', extensions: ['exe'] }]
        : [{ name: '归档', extensions: ['zst', 'enc'] }]
    });
    if (typeof selected === 'string') output = selected;
  }

  async function submit(): Promise<void> {
    touched = true;
    if (!source) {
      app.setStatus('请先选择压缩源路径。', 'error');
      toasts.warn('还没有选择源路径', '点击「文件」或「目录」，也可以直接把文件拖进窗口。');
      return;
    }
    const pw = encrypt ? emptyToNull(password) : null;
    if (encrypt && !pw) {
      app.setStatus('启用加密时必须输入密码。', 'error');
      toasts.warn('缺少密码', '密码一旦遗失，归档无法恢复，请务必妥善保存。');
      return;
    }

    const ok = await task.run('compress', `正在压缩 ${pathBaseName(source)}...`, async () => {
      report = await api.compress({
        sourcePath: source,
        outputPath: emptyToNull(output),
        outputKind,
        level,
        includeRootDir,
        password: pw,
        splitSizeMib: isSfx || splitSize <= 0 ? null : splitSize,
        enableLogging,
        deleteSourceAfter,
        threads: threads > 0 ? threads : null
      });
      app.setStatus(`压缩完成: ${report.outputPath}`, 'success');
    });

    if (ok && report) {
      toasts.success(
        '压缩完成',
        report.sidecarPath
          ? '同目录下的 .payload 是数据文件，分发时请一并携带。'
          : pathBaseName(report.outputPath)
      );
    }
  }
</script>

<div class="flex flex-col gap-4 animate-[var(--animate-rise)]">
  <Card title="压缩源" subtitle="选择要打包的文件或整个目录" icon="folder">
    {#snippet actions()}
      <Tag tone={source ? 'accent' : 'neutral'}>{source ? kind : '未选择'}</Tag>
    {/snippet}

    <div class="flex flex-col gap-4">
      <Field error={sourceError}>
        {#snippet aside()}
          {#if app.compressInfoLoading}
            正在统计体积...
          {:else if info?.exists}
            {formatBytes(info.sizeBytes)}{info.isDir
              ? ` · ${formatCount(info.fileCount)} 个文件`
              : ''}{info.truncated ? '（已达统计上限，实际更多）' : ''}
          {:else if source}
            路径不存在
          {/if}
        {/snippet}
        <PathInput
          value={source}
          onCommit={(next) => app.setCompressSource(next, kind)}
          icon={kind === '目录' ? 'folder' : 'file'}
          invalid={Boolean(sourceError)}
          placeholder="把文件拖进窗口，或点击右侧按钮选择"
        >
          {#snippet actions()}
            <Button variant="ghost" size="sm" icon="file" onclick={pickFile}>文件</Button>
            <Button variant="ghost" size="sm" icon="folder" onclick={pickDirectory}>目录</Button>
          {/snippet}
        </PathInput>
      </Field>

      <Field
        label="输出路径"
        hint="留空则在源路径旁自动命名，扩展名按输出类型决定。"
      >
        <PathInput bind:value={output} icon="archive" placeholder="留空则自动生成">
          {#snippet actions()}
            <Button variant="ghost" size="sm" icon="folder" onclick={pickOutput}>选择</Button>
          {/snippet}
        </PathInput>
      </Field>
    </div>
  </Card>

  <Card title="压缩参数" subtitle="等级越高体积越小，耗时也越长" icon="sliders">
    <div class="flex flex-col gap-5">
      <Field label="输出类型" hint={isSfx
        ? '自解压 EXE 生成单个可执行文件，双击后进入解压界面；当前版本不支持分卷。'
        : '普通归档保留分卷能力，加密时输出 .enc。'}>
        <Segmented bind:value={outputKind} options={outputOptions} ariaLabel="输出类型" />
      </Field>

      <Field label="压缩等级" hint={levelHint}>
        {#snippet aside()}
          <span class="text-sm font-bold text-accent tabular-nums">{level}</span>
        {/snippet}
        <Slider
          bind:value={app.compressLevel}
          min={1}
          max={22}
          marks={levelMarks}
          ariaLabel="压缩等级"
        />
      </Field>

      <div class="grid grid-cols-2 gap-2 rounded-panel bg-inset p-1.5">
        <Toggle
          bind:checked={includeRootDir}
          label="包含根目录"
          description="解压后多一层同名目录"
          icon="layers"
          disabled={kind === '文件'}
        />
        <Toggle
          bind:checked={encrypt}
          label="启用加密"
          description="Argon2id + XChaCha20-Poly1305"
          icon="shield"
        />
      </div>

      {#if encrypt}
        <Field label="密码" error={passwordError}>
          <PasswordInput
            bind:value={password}
            showStrength
            placeholder="设置一个只有你知道的密码"
          />
        </Field>
      {/if}

      <div class="border-t border-line pt-1">
        <button
          type="button"
          onclick={() => (advanced = !advanced)}
          aria-expanded={advanced}
          class="flex w-full items-center gap-2 rounded-control px-1 py-2 text-xs font-semibold text-fg-soft transition-colors hover:text-fg"
        >
          <span class="transition-transform duration-200 {advanced ? 'rotate-90' : ''}">
            <Icon name="chevronRight" size={14} />
          </span>
          高级选项
          <span class="ml-auto font-normal text-fg-faint">分卷 · 线程 · 日志 · 删除源</span>
        </button>

        {#if advanced}
          <div class="flex flex-col gap-4 pt-3">
            <div class="grid grid-cols-2 gap-4">
              <Field
                label="分卷大小"
                hint={isSfx ? '自解压模式下不可用。' : '0 表示不分卷。'}
              >
                <NumberInput bind:value={splitSize} suffix="MiB" min={0} disabled={isSfx} />
              </Field>
              <Field label="工作线程" hint="0 表示使用全部可用核心。">
                <NumberInput bind:value={threads} suffix="线程" min={0} max={256} />
              </Field>
            </div>

            <div class="grid grid-cols-2 gap-2 rounded-panel bg-inset p-1.5">
              <Toggle
                bind:checked={enableLogging}
                label="写入日志"
                description="在输出目录留下运行记录"
                icon="text"
              />
              <Toggle
                bind:checked={deleteSourceAfter}
                label="完成后删除源"
                description="不可撤销，请谨慎开启"
                icon="trash"
                danger
              />
            </div>
          </div>
        {/if}
      </div>

      <div class="flex items-center gap-3 border-t border-line pt-4">
        <Button icon="play" loading={running} disabled={busy && !running} onclick={submit}>
          开始压缩
        </Button>
        {#if running}
          <Button variant="danger" icon="stop" disabled={task.aborting} onclick={() => task.requestAbort()}>
            {task.aborting ? '正在停止' : '停止'}
          </Button>
        {/if}
        <span class="ml-auto text-[0.7rem] text-fg-faint">Ctrl + Enter</span>
      </div>
    </div>
  </Card>

  {#if report}
    <ResultCard title="压缩完成" {report} onDismiss={() => (report = null)} />
  {/if}
</div>

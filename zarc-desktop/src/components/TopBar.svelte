<script lang="ts">
  import { app, type StatusLevel } from '../stores/app.svelte';
  import { theme } from '../stores/theme.svelte';
  import type { ViewId } from '../lib/api';
  import Icon, { type IconName } from './ui/Icon.svelte';
  import Spinner from './ui/Spinner.svelte';

  const meta: Record<ViewId, { title: string; subtitle: string }> = {
    compress: { title: '压缩存档', subtitle: '把文件或目录打包为 zstd 归档、加密包或自解压 EXE' },
    decompress: { title: '解压还原', subtitle: '预览归档内容并还原到指定目录' },
    benchmark: { title: '性能测试', subtitle: '在真实样本上试跑各等级，选出速度与体积的平衡点' }
  };

  let view = $derived(app.currentView);
  let title = $derived(app.isSfx ? '自解压模式' : meta[view].title);
  let subtitle = $derived(
    app.isSfx ? '这个可执行文件内嵌了归档数据，选择输出目录即可释放' : meta[view].subtitle
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

    <!-- 自解压模式下没有侧边栏，主题开关得在这里也留一个入口。 -->
    {#if app.isSfx}
      <button
        type="button"
        onclick={() => theme.toggle()}
        aria-label="切换主题"
        title="切换主题"
        class="panel flex h-8 w-8 items-center justify-center rounded-pill text-fg-soft transition-colors hover:text-fg"
      >
        <Icon name={theme.current === 'dark' ? 'moon' : 'sun'} size={15} />
      </button>
    {/if}
  </div>
</header>

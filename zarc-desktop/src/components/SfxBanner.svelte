<script lang="ts">
  import { app } from '../stores/app.svelte';
  import { formatBytes } from '../lib/format';
  import Icon from './ui/Icon.svelte';
  import Tag from './ui/Tag.svelte';

  let info = $derived(app.sfxInfo);
</script>

{#if info}
  <div
    class="panel flex items-start gap-3.5 rounded-panel border-accent/30 bg-accent-wash p-4 animate-[var(--animate-rise)]"
  >
    <span
      class="flex h-10 w-10 shrink-0 items-center justify-center rounded-control bg-accent text-accent-fg shadow-[var(--shadow-glow)]"
    >
      <Icon name="archive" size={19} />
    </span>

    <div class="flex min-w-0 flex-col gap-1.5">
      <p class="text-sm font-bold text-fg">这是一个自解压包</p>
      <p class="text-xs leading-relaxed text-fg-soft">
        归档数据已内嵌在本程序中，选择输出目录即可释放，无需另外准备 ZARC。
      </p>
      <div class="mt-0.5 flex flex-wrap items-center gap-1.5">
        <Tag tone="accent">{info.archiveKind}</Tag>
        <Tag tone="neutral">{formatBytes(info.payloadBytes)}</Tag>
        <Tag tone="neutral" icon="folder">{info.defaultExtractName}</Tag>
        {#if info.encrypted}
          <Tag tone="warning" icon="shield">已加密</Tag>
        {/if}
      </div>
    </div>
  </div>
{/if}

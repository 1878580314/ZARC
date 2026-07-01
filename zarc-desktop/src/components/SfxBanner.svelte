<script lang="ts">
  import { app } from '../stores/app.svelte';
  import { formatBytes } from '../lib/format';

  let info = $derived(app.sfxInfo);
</script>

{#if info}
  <div
    class="flex items-center gap-3 rounded-2xl border border-accent/30 bg-accent/10 px-4 py-3 animate-[var(--animate-fade-in)]"
  >
    <span class="text-2xl">📦</span>
    <div class="flex flex-col">
      <span class="text-sm font-semibold text-primary">当前文件为自解压包</span>
      <span class="text-xs text-secondary">
        {info.archiveKind} • {formatBytes(info.payloadBytes)} • 默认输出 {info.defaultExtractName}{info.encrypted
          ? ' • 已加密'
          : ''}
      </span>
    </div>
  </div>
{/if}

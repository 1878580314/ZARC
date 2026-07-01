<script lang="ts">
  import { app } from '../stores/app.svelte';

  const titles: Record<string, string> = {
    compress: '压缩存档',
    decompress: '解压还原',
    benchmark: '性能测试'
  };

  let title = $derived(app.isSfx ? '自解压模式' : titles[app.currentView] ?? 'ZARC');
  let status = $derived(app.status);

  const levelClass = {
    idle: 'text-muted',
    busy: 'text-accent',
    success: 'text-success',
    error: 'text-danger'
  };
</script>

<header class="flex items-center justify-between px-2 py-1">
  <h1 class="text-xl font-bold text-primary">{title}</h1>
  <div
    class="flex items-center gap-2 rounded-full bg-[var(--surface)] px-4 py-1.5 text-xs font-medium {levelClass[status.level]}"
  >
    {#if status.level === 'busy'}
      <span class="h-3 w-3 animate-[var(--animate-spin-slow)] rounded-full border-2 border-accent border-t-transparent"></span>
    {/if}
    <span>{status.message}</span>
  </div>
</header>

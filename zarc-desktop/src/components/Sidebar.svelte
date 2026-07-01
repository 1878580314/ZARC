<script lang="ts">
  import { app } from '../stores/app.svelte';
  import { theme } from '../stores/theme.svelte';
  import type { ViewId } from '../lib/api';

  const navItems: { id: ViewId; label: string; icon: string }[] = [
    { id: 'compress', label: '压缩', icon: '🗜️' },
    { id: 'decompress', label: '解压', icon: '📂' },
    { id: 'benchmark', label: '测试', icon: '⚡' }
  ];

  let current = $derived(app.currentView);
</script>

<aside class="flex h-full w-60 shrink-0 flex-col gap-2 p-4">
  <div class="flex items-center gap-3 px-2 py-4">
    <div
      class="flex h-10 w-10 items-center justify-center rounded-2xl bg-accent text-xl shadow-[var(--shadow-glow)]"
    >
      🗜️
    </div>
    <div class="flex flex-col">
      <span class="text-base font-extrabold tracking-tight text-primary">ZARC</span>
      <span class="text-[0.65rem] text-muted">Rapid Compressor</span>
    </div>
  </div>

  <nav class="mt-2 flex flex-col gap-1.5">
    {#each navItems as item (item.id)}
      <button
        onclick={() => app.setView(item.id)}
        class="group flex items-center gap-3 rounded-2xl px-3 py-2.5 text-sm font-medium transition-all duration-200 {current ===
        item.id
          ? 'bg-accent text-white shadow-[var(--shadow-glow)]'
          : 'text-secondary hover:bg-[var(--surface-hover)] hover:text-primary'}"
      >
        <span class="text-lg transition-transform duration-200 group-hover:scale-110">{item.icon}</span>
        {item.label}
      </button>
    {/each}
  </nav>

  <div class="mt-auto">
    <button
      onclick={() => theme.toggle()}
      class="flex w-full items-center gap-3 rounded-2xl px-3 py-2.5 text-sm font-medium text-secondary transition-colors hover:bg-[var(--surface-hover)] hover:text-primary"
    >
      <span class="text-lg">{theme.current === 'dark' ? '🌙' : '☀️'}</span>
      {theme.current === 'dark' ? '深色模式' : '浅色模式'}
    </button>
  </div>
</aside>

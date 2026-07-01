<script lang="ts">
  import type { ArchiveContentReport } from '../lib/api';
  import { formatBytes, getFileIcon } from '../lib/format';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';

  interface Props {
    report: ArchiveContentReport;
    onClose: () => void;
  }

  let { report, onClose }: Props = $props();

  let currentPath = $state('');

  interface VisibleItem {
    name: string;
    size: number;
    isDir: boolean;
  }

  let visibleItems = $derived.by<VisibleItem[]>(() => {
    const items = new Map<string, VisibleItem>();
    for (const entry of report.entries) {
      const relPath = entry.path.startsWith(currentPath)
        ? entry.path.slice(currentPath.length)
        : null;
      if (relPath === null || relPath === '') continue;
      const parts = relPath.split('/');
      const name = parts[0];
      const isDir = parts.length > 1 || entry.isDir;
      const existing = items.get(name);
      if (existing) {
        existing.size += entry.size;
        if (isDir) existing.isDir = true;
      } else {
        items.set(name, { name, size: entry.size, isDir });
      }
    }
    return Array.from(items.values()).sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
  });

  let crumbs = $derived.by<{ label: string; path: string }[]>(() => {
    const list: { label: string; path: string }[] = [{ label: 'root', path: '' }];
    if (currentPath) {
      const parts = currentPath.split('/').filter((p) => p);
      let cumulative = '';
      for (const p of parts) {
        cumulative += p + '/';
        list.push({ label: p, path: cumulative });
      }
    }
    return list;
  });

  function enter(name: string) {
    currentPath += name + '/';
  }

  function goto(path: string) {
    currentPath = path;
  }
</script>

<Card>
  <div class="mb-4 flex items-center justify-between">
    <h3 class="text-base font-bold text-primary">归档预览</h3>
    <Button variant="ghost" onclick={onClose}>关闭</Button>
  </div>

  <div class="mb-3 flex flex-wrap items-center gap-1 text-xs">
    {#each crumbs as crumb, i (crumb.path)}
      {#if i > 0}
        <span class="text-muted">/</span>
      {/if}
      <button
        onclick={() => goto(crumb.path)}
        class="rounded-md px-1.5 py-0.5 font-medium text-secondary transition-colors hover:bg-[var(--surface-hover)] hover:text-accent"
      >
        {crumb.label}
      </button>
    {/each}
  </div>

  <div class="mb-3 flex flex-wrap gap-4 text-xs text-muted">
    <span>文件总数: <strong class="text-secondary">{report.totalFiles}</strong></span>
    <span>解压总计: <strong class="text-secondary">{formatBytes(report.uncompressedSize)}</strong></span>
    <span class="truncate">BLAKE3: <strong class="text-secondary">{report.hash}</strong></span>
  </div>

  <div class="max-h-80 overflow-y-auto rounded-2xl bg-[var(--surface-hover)] p-1">
    {#if visibleItems.length === 0}
      <p class="px-3 py-6 text-center text-sm text-muted">空目录</p>
    {:else}
      <ul class="flex flex-col">
        {#each visibleItems as item (item.name)}
          <li
            class="flex items-center justify-between rounded-xl px-3 py-2 text-sm transition-colors {item.isDir
              ? 'cursor-pointer hover:bg-[var(--surface)]'
              : ''}"
          >
            {#if item.isDir}
              <button
                type="button"
                class="flex flex-1 items-center justify-between"
                onclick={() => enter(item.name)}
              >
                <div class="flex items-center gap-2.5">
                  <span class="text-base">📁</span>
                  <span class="font-medium text-primary">{item.name}</span>
                </div>
                <span class="text-xs text-muted tabular-nums">-</span>
              </button>
            {:else}
              <div class="flex flex-1 items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <span class="text-base">{getFileIcon(item.name)}</span>
                  <span class="font-medium text-primary">{item.name}</span>
                </div>
                <span class="text-xs text-muted tabular-nums">{formatBytes(item.size)}</span>
              </div>
            {/if}
            <div class="flex items-center gap-2.5">
              <span class="text-base">{item.isDir ? '📁' : getFileIcon(item.name)}</span>
              <span class="font-medium text-primary">{item.name}</span>
            </div>
            <span class="text-xs text-muted tabular-nums">
              {item.isDir ? '-' : formatBytes(item.size)}
            </span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</Card>

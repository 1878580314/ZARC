<script lang="ts">
  import type { ArchiveContentReport } from '../lib/api';
  import { formatBytes, formatCount, getFileIcon } from '../lib/format';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';
  import Icon, { type IconName } from './ui/Icon.svelte';
  import CopyButton from './ui/CopyButton.svelte';

  interface Props {
    report: ArchiveContentReport;
    onClose: () => void;
  }

  let { report, onClose }: Props = $props();

  let currentPath = $state('');
  let query = $state('');

  interface Row {
    /** 目录模式下是条目名，搜索模式下是完整相对路径。 */
    key: string;
    name: string;
    size: number;
    isDir: boolean;
    /** 目录里包含的文件数；文件恒为 1。 */
    files: number;
  }

  let searching = $derived(query.trim().length > 0);

  /** 搜索是全局的：不受当前目录限制，否则用户得先猜对目录才能找到东西。 */
  let searchRows = $derived.by<Row[]>(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return [];
    const rows: Row[] = [];
    for (const entry of report.entries) {
      if (entry.isDir) continue;
      if (!entry.path.toLowerCase().includes(needle)) continue;
      rows.push({ key: entry.path, name: entry.path, size: entry.size, isDir: false, files: 1 });
      if (rows.length >= 500) break;
    }
    return rows;
  });

  let browseRows = $derived.by<Row[]>(() => {
    const items = new Map<string, Row>();
    for (const entry of report.entries) {
      if (!entry.path.startsWith(currentPath)) continue;
      const relPath = entry.path.slice(currentPath.length);
      if (relPath === '') continue;
      const parts = relPath.split('/');
      const name = parts[0];
      const isDir = parts.length > 1 || entry.isDir;
      const existing = items.get(name);
      if (existing) {
        existing.size += entry.size;
        if (isDir) existing.isDir = true;
        if (!entry.isDir) existing.files += 1;
      } else {
        items.set(name, { key: name, name, size: entry.size, isDir, files: entry.isDir ? 0 : 1 });
      }
    }
    return Array.from(items.values()).sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      return a.name.localeCompare(b.name, 'zh-CN');
    });
  });

  let rows = $derived(searching ? searchRows : browseRows);
  let truncated = $derived(searching && searchRows.length >= 500);

  let crumbs = $derived.by<{ label: string; path: string }[]>(() => {
    const list = [{ label: '归档根目录', path: '' }];
    let cumulative = '';
    for (const part of currentPath.split('/').filter(Boolean)) {
      cumulative += part + '/';
      list.push({ label: part, path: cumulative });
    }
    return list;
  });

  function enter(name: string): void {
    currentPath += name + '/';
  }

  function goUp(): void {
    const parts = currentPath.split('/').filter(Boolean);
    parts.pop();
    currentPath = parts.length > 0 ? parts.join('/') + '/' : '';
  }

  const stats = $derived([
    { label: '文件总数', value: formatCount(report.totalFiles) },
    { label: '解压后体积', value: formatBytes(report.uncompressedSize) }
  ]);

  const rowClass =
    'flex w-full items-center gap-2.5 rounded-control px-3 py-2 text-left text-sm transition-colors';
</script>

{#snippet rowBody(row: Row)}
  <span class="shrink-0 {row.isDir ? 'text-accent' : 'text-fg-faint'}">
    <Icon name={row.isDir ? 'folder' : (getFileIcon(row.name) as IconName)} size={16} />
  </span>
  <span class="min-w-0 flex-1 truncate font-medium text-fg" title={row.name}>{row.name}</span>
  <span class="shrink-0 text-xs text-fg-faint tabular-nums">
    {#if row.isDir}
      {formatCount(row.files)} 项 · {formatBytes(row.size)}
    {:else}
      {formatBytes(row.size)}
    {/if}
  </span>
{/snippet}

<Card title="归档预览" subtitle="只读浏览，不会写入磁盘" icon="layers">
  {#snippet actions()}
    <Button variant="subtle" size="sm" icon="close" onclick={onClose}>关闭</Button>
  {/snippet}

  <div class="flex flex-col gap-3">
    <div class="grid grid-cols-2 gap-2">
      {#each stats as stat (stat.label)}
        <div class="rounded-control bg-inset px-3 py-2">
          <p class="text-[0.65rem] tracking-wide text-fg-faint">{stat.label}</p>
          <p class="mt-0.5 text-sm font-bold text-fg tabular-nums">{stat.value}</p>
        </div>
      {/each}
    </div>

    {#if report.hash}
      <div class="flex items-center gap-2 rounded-control bg-inset px-3 py-2">
        <span class="shrink-0 text-[0.65rem] tracking-wide text-fg-faint">BLAKE3</span>
        <code class="mono min-w-0 flex-1 truncate text-[0.7rem] text-fg-soft" data-selectable>
          {report.hash}
        </code>
        <CopyButton text={report.hash} label="复制哈希" />
      </div>
    {/if}

    <label class="relative block">
      <span class="sr-only">搜索归档内的文件</span>
      <span class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-fg-faint">
        <Icon name="search" size={15} />
      </span>
      <input
        class="control py-2 pr-9 pl-9 text-sm"
        placeholder="搜索文件名或路径..."
        bind:value={query}
      />
      {#if searching}
        <button
          type="button"
          class="absolute top-1/2 right-2 -translate-y-1/2 rounded-control p-1 text-fg-faint transition-colors hover:text-fg"
          aria-label="清除搜索"
          onclick={() => (query = '')}
        >
          <Icon name="close" size={14} />
        </button>
      {/if}
    </label>

    {#if searching}
      <p class="text-xs text-fg-faint">
        匹配 {formatCount(searchRows.length)} 个文件{truncated ? '（仅显示前 500 条）' : ''}
      </p>
    {:else}
      <nav class="flex flex-wrap items-center gap-0.5 text-xs" aria-label="归档路径">
        {#each crumbs as crumb, i (crumb.path)}
          {#if i > 0}
            <Icon name="chevronRight" size={12} class="text-fg-faint" />
          {/if}
          <button
            type="button"
            onclick={() => (currentPath = crumb.path)}
            disabled={i === crumbs.length - 1}
            class="rounded-control px-1.5 py-0.5 font-medium transition-colors {i ===
            crumbs.length - 1
              ? 'text-fg'
              : 'text-fg-soft hover:bg-inset hover:text-accent'}"
          >
            {crumb.label}
          </button>
        {/each}
      </nav>
    {/if}

    <div class="max-h-80 overflow-y-auto rounded-control bg-inset p-1">
      {#if !searching && currentPath}
        <button
          type="button"
          onclick={goUp}
          class="flex w-full items-center gap-2.5 rounded-control px-3 py-2 text-sm text-fg-soft transition-colors hover:bg-panel-solid hover:text-fg"
        >
          <Icon name="chevronLeft" size={16} />
          <span class="font-medium">返回上一级</span>
        </button>
      {/if}

      {#if rows.length === 0}
        <p class="px-3 py-8 text-center text-sm text-fg-faint">
          {searching ? '没有匹配的文件' : '这个目录是空的'}
        </p>
      {:else}
        <ul class="flex flex-col">
          {#each rows as row (row.key)}
            <li>
              <!-- 目录可点进去，文件不可点：两者用不同标签，避免给静态行套上按钮语义。 -->
              {#if row.isDir}
                <button
                  type="button"
                  onclick={() => enter(row.name)}
                  class="{rowClass} cursor-pointer hover:bg-panel-solid"
                >
                  {@render rowBody(row)}
                </button>
              {:else}
                <div class={rowClass}>{@render rowBody(row)}</div>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</Card>

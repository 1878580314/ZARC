<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { dropOverlay } from '../lib/dragdrop.svelte';
  import Icon from './ui/Icon.svelte';
</script>

{#if dropOverlay.visible}
  <div
    class="fixed inset-0 z-[9999] flex items-center justify-center bg-canvas/55 backdrop-blur-md"
    transition:fade={{ duration: 140 }}
  >
    <div
      class="panel-solid flex flex-col items-center gap-4 rounded-[2rem] px-16 py-12 text-center shadow-[var(--shadow-lift)] outline-2 outline-offset-[-10px] outline-dashed outline-accent/55"
      in:scale={{ duration: 200, start: 0.94 }}
      out:scale={{ duration: 120, start: 0.97 }}
    >
      <span
        class="flex h-16 w-16 animate-[var(--animate-float)] items-center justify-center rounded-full bg-accent-wash text-accent"
      >
        <Icon name="dropIn" size={30} stroke={1.6} />
      </span>
      <div class="flex flex-col gap-1">
        <p class="text-lg font-bold tracking-tight text-fg">
          {#if dropOverlay.count > 1}
            放开以载入 {dropOverlay.count} 个文件
          {:else}
            放开以载入
          {/if}
        </p>
        <p class="text-sm text-fg-soft">
          归档（.zst / .enc / .001 / .exe）会进入解压页，其余进入压缩页
        </p>
      </div>
      {#if dropOverlay.count > 1}
        <p class="text-xs text-warning">一次只能处理一个路径，将使用第一个。</p>
      {/if}
    </div>
  </div>
{/if}

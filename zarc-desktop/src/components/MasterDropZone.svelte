<script lang="ts">
  import { fade } from 'svelte/transition';
  import { dropOverlay } from '../lib/dragdrop';
</script>

{#if dropOverlay.visible}
  <div
    class="fixed inset-0 z-[9999] flex items-center justify-center bg-accent/10 backdrop-blur-sm"
    transition:fade={{ duration: 150 }}
  >
    <div
      class="flex flex-col items-center gap-4 rounded-[2rem] border-2 border-dashed border-accent/60 bg-[var(--surface)] px-16 py-12 shadow-[var(--shadow-glow)]"
    >
      <div class="text-5xl animate-[var(--animate-bounce-soft)]">📥</div>
      <div class="text-lg font-semibold text-primary">拖放文件到此处</div>
      <div class="text-sm text-secondary">支持归档文件 (.zst/.enc/.001/.exe) 或待压缩文件</div>
    </div>
  </div>
{/if}

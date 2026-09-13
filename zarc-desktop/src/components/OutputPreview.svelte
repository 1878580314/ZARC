<script lang="ts">
  import { api, type OutputKind } from '../lib/api';
  import { emptyToNull } from '../lib/format';
  import { t } from '../lib/i18n/index.svelte';
  let { source, output, decompress = false, encrypted = false, outputKind = 'archive', splitSizeMib = 0 }:
    { source: string; output: string; decompress?: boolean; encrypted?: boolean; outputKind?: OutputKind; splitSizeMib?: number } = $props();
  let resolved = $state('');
  $effect(() => {
    const request = { sourcePath: source, outputPath: emptyToNull(output), decompress, encrypted, outputKind, splitSizeMib };
    let active = true;
    resolved = '';
    if (!source.trim()) return;
    const timer = setTimeout(() => {
      void api.previewOutput(request).then((path) => { if (active) resolved = path; }).catch(() => {});
    }, 200);
    return () => { active = false; clearTimeout(timer); };
  });
</script>

{#if resolved}
  <div class="rounded-control bg-inset px-4 py-3 text-xs text-fg-soft" aria-live="polite">
    <p class="font-semibold">{t('audit.finalOutput')}</p>
    <p class="mt-1 break-all" data-selectable>{resolved}</p>
    {#if decompress}<p class="mt-1">{t('audit.outputHint')}</p>{/if}
  </div>
{/if}

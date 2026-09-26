<script lang="ts">
  import { api, type OperationReport } from '../lib/api';
  import { toasts } from '../stores/toast.svelte';
  import { normalizeError } from '../lib/format';
  import { operationFields, operationHighlights } from '../lib/format';
  import { t } from '../lib/i18n/index.svelte';
  import Card from './ui/Card.svelte';
  import Button from './ui/Button.svelte';
  import CopyButton from './ui/CopyButton.svelte';

  interface Props {
    title: string;
    report: OperationReport;
    onDismiss: () => void;
  }

  let { title, report, onDismiss }: Props = $props();

  let highlights = $derived(operationHighlights(report));
  let fields = $derived(operationFields(report));

  /** 一键复制整份报告，可直接粘贴到工单或聊天窗口。 / Copy the whole report in one go, ready to paste into a ticket or chat window. */
  let plainText = $derived(fields.map((f) => `${f.label}: ${f.value}`).join('\n'));
</script>

<Card {title} icon="checkCircle" subtitle={report.operation} class="animate-[var(--animate-rise)]">
  {#snippet actions()}
    <Button variant="subtle" size="sm" icon="folder" onclick={() => api.revealOutput(report.outputPath).catch((e) => toasts.error(t('audit.openFailed'), normalizeError(e)))}>{t('audit.openFolder')}</Button>
    <CopyButton text={plainText} label={t('shell.copyFullReport')} />
    <Button variant="subtle" size="sm" onclick={onDismiss}>{t('shell.close')}</Button>
  {/snippet}

  <div class="grid grid-cols-3 gap-3">
    {#each highlights as item (item.label)}
      <div class="rounded-control bg-inset px-3 py-3 text-center">
        <div class="text-[0.7rem] tracking-wide text-fg-faint">{item.label}</div>
        <div class="mt-1 text-lg font-extrabold tracking-tight text-fg tabular-nums">
          {item.value}
        </div>
      </div>
    {/each}
  </div>

  <!-- 定义列表让每一项都能独立对齐和复制。 / A definition list lets every item align and copy on its own. -->
  <dl class="mt-4 flex flex-col divide-y divide-line text-xs">
    {#each fields as field (field.label)}
      <div class="flex items-start gap-3 py-2">
        <dt class="w-20 shrink-0 pt-px text-fg-faint">{field.label}</dt>
        <dd
          class="min-w-0 flex-1 break-all text-fg-soft {field.mono ? 'mono text-[0.7rem]' : ''}"
          data-selectable
        >
          {field.value}
        </dd>
        {#if field.mono}
          <CopyButton text={field.value} label={t('shell.copyField', { label: field.label })} />
        {/if}
      </div>
    {/each}
  </dl>
</Card>

<script lang="ts">
  import type { OperationReport } from '../lib/api';
  import { operationFields, operationHighlights } from '../lib/format';
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

  /** 一次性复制整份报告，便于贴进工单或聊天窗口。 */
  let plainText = $derived(fields.map((f) => `${f.label}: ${f.value}`).join('\n'));
</script>

<Card {title} icon="checkCircle" subtitle={report.operation} class="animate-[var(--animate-rise)]">
  {#snippet actions()}
    <CopyButton text={plainText} label="复制完整报告" />
    <Button variant="subtle" size="sm" onclick={onDismiss}>关闭</Button>
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

  <!--
    旧实现把整份报告拼成字符串塞进 <pre>：路径无法单独复制、哈希会撑爆横向
    滚动条。改成定义列表后每一项都能独立对齐与复制。
  -->
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
          <CopyButton text={field.value} label="复制 {field.label}" />
        {/if}
      </div>
    {/each}
  </dl>
</Card>

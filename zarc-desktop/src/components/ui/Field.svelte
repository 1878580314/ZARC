<script lang="ts">
  import { setContext, type Snippet } from 'svelte';
  import { FIELD, type FieldContext } from './field';
  import Icon from './Icon.svelte';

  interface Props {
    label?: string;
    /** 控件正常状态下显示在其下方的辅助文本。 / Helper text shown below the control in its normal state. */
    hint?: string;
    /** 校验失败消息；出现时替代辅助文本并转为警示色。 / Validation failure message; when present it replaces the hint and turns danger-colored. */
    error?: string;
    /** 标题行右侧的补充信息（如测量大小、密码强度）。 / Supplementary info on the right of the label row (e.g. measured size, password strength). */
    aside?: Snippet;
    class?: string;
    children: Snippet;
  }

  let { label, hint, error, aside, class: klass = '', children }: Props = $props();
  const id = $props.id();
  setContext<FieldContext>(FIELD, { id, description: () => error || hint ? `${id}-help` : undefined, invalid: () => Boolean(error) });
</script>

<div class="flex flex-col gap-1.5 {klass}">
  {#if label || aside}
    <div class="flex items-baseline justify-between gap-3">
      {#if label}
        <label class="field-label" for={id}>{label}</label>
      {/if}
      {#if aside}
        <span class="min-w-0 text-xs text-fg-faint">{@render aside()}</span>
      {/if}
    </div>
  {/if}

  {@render children()}

  {#if error}
    <p id={`${id}-help`} role="alert" class="flex items-start gap-1.5 text-xs text-danger">
      <Icon name="error" size={13} class="mt-px" />
      <span>{error}</span>
    </p>
  {:else if hint}
    <p id={`${id}-help`} class="text-xs leading-relaxed text-fg-faint">{hint}</p>
  {/if}
</div>

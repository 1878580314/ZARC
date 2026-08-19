<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  interface Props {
    label?: string;
    /** 常态说明文字，出现在控件下方。 */
    hint?: string;
    /** 校验失败信息；出现时取代 hint 并转为危险色。 */
    error?: string;
    /** 标题行右侧的补充信息（例如实测体积、密码强度）。 */
    aside?: Snippet;
    class?: string;
    children: Snippet;
  }

  let { label, hint, error, aside, class: klass = '', children }: Props = $props();
</script>

<div class="flex flex-col gap-1.5 {klass}">
  {#if label || aside}
    <div class="flex items-baseline justify-between gap-3">
      {#if label}
        <span class="field-label">{label}</span>
      {/if}
      {#if aside}
        <span class="min-w-0 text-xs text-fg-faint">{@render aside()}</span>
      {/if}
    </div>
  {/if}

  {@render children()}

  {#if error}
    <p class="flex items-start gap-1.5 text-xs text-danger">
      <Icon name="error" size={13} class="mt-px" />
      <span>{error}</span>
    </p>
  {:else if hint}
    <p class="text-xs leading-relaxed text-fg-faint">{hint}</p>
  {/if}
</div>

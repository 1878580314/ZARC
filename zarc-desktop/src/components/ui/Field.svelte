<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  interface Props {
    label?: string;
    /** Helper text shown below the control in its normal state. */
    hint?: string;
    /** Validation failure message; when present it replaces the hint and turns danger-colored. */
    error?: string;
    /** Supplementary info on the right of the label row (e.g. measured size, password strength). */
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

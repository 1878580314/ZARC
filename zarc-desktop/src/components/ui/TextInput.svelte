<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';

  interface Props extends HTMLInputAttributes {
    value?: string;
  }

  let { value = $bindable(''), class: klass = '', ...rest }: Props = $props();
</script>

<input
  bind:value
  class="w-full rounded-2xl border border-[var(--border-input)] bg-[var(--surface-solid)] px-4 py-2.5 text-sm text-primary placeholder:text-muted transition-colors focus:border-accent focus:outline-none focus:ring-2 focus:ring-accent/20 disabled:cursor-not-allowed disabled:opacity-50 {klass}"
  {...rest}
/>

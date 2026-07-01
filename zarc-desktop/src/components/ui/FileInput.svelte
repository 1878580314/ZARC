<script lang="ts">
  interface Props {
    value: string;
    placeholder?: string;
    disabled?: boolean;
    class?: string;
  }

  let { value = $bindable(), placeholder = '', disabled = false, class: klass = '' }: Props = $props();
</script>

<input
  type="text"
  bind:value
  {placeholder}
  {disabled}
  readonly
  class="w-full rounded-2xl border border-[var(--border-input)] bg-[var(--surface-solid)] px-4 py-2.5 text-sm text-primary placeholder:text-muted transition-colors focus:border-accent focus:outline-none focus:ring-2 focus:ring-accent/20 disabled:cursor-not-allowed disabled:opacity-50 {klass}"
/>

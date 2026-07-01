<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';

  interface Props extends HTMLInputAttributes {
    value?: number;
    label?: string;
    suffix?: string;
  }

  let { value = $bindable(0), label, suffix, class: klass = '', ...rest }: Props = $props();

  function handle(e: Event) {
    const target = e.target as HTMLInputElement;
    const parsed = Number.parseInt(target.value, 10);
    value = Number.isFinite(parsed) ? parsed : 0;
  }
</script>

<div class="flex flex-col gap-1.5">
  {#if label}
    <span class="text-xs font-medium text-secondary">{label}</span>
  {/if}
  <div class="relative">
    <input
      type="number"
      {value}
      oninput={handle}
      class="w-full rounded-2xl border border-[var(--border-input)] bg-[var(--surface-solid)] px-4 py-2.5 text-sm text-primary transition-colors focus:border-accent focus:outline-none focus:ring-2 focus:ring-accent/20 {suffix ? 'pr-14' : ''} {klass}"
      {...rest}
    />
    {#if suffix}
      <span class="absolute top-1/2 right-4 -translate-y-1/2 text-xs text-muted">{suffix}</span>
    {/if}
  </div>
</div>

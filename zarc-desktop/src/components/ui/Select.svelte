<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte';
  import type { HTMLSelectAttributes } from 'svelte/elements';

  interface Props extends HTMLSelectAttributes {
    value: T;
    options: { value: T; label: string }[];
    children?: Snippet;
  }

  let { value = $bindable(), options, class: klass = '', children, ...rest }: Props = $props();

  function handle(e: Event) {
    value = (e.target as HTMLSelectElement).value as T;
  }
</script>

<div class="relative">
  <select
    {value}
    onchange={handle}
    class="w-full cursor-pointer appearance-none rounded-2xl border border-[var(--border-input)] bg-[var(--surface-solid)] px-4 py-2.5 pr-10 text-sm font-medium text-primary transition-colors focus:border-accent focus:outline-none focus:ring-2 focus:ring-accent/20 {klass}"
    {...rest}
  >
    {#each options as opt (opt.value)}
      <option value={opt.value}>{opt.label}</option>
    {/each}
  </select>
  <svg
    class="pointer-events-none absolute top-1/2 right-4 h-4 w-4 -translate-y-1/2 text-muted"
    viewBox="0 0 20 20"
    fill="currentColor"
  >
    <path
      fill-rule="evenodd"
      d="M5.23 7.21a.75.75 0 011.06.02L10 11.17l3.71-3.94a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z"
      clip-rule="evenodd"
    />
  </svg>
</div>

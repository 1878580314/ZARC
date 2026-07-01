<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  type Variant = 'primary' | 'danger' | 'ghost';

  interface Props extends HTMLButtonAttributes {
    variant?: Variant;
    class?: string;
    children: Snippet;
  }

  let {
    variant = 'primary',
    class: klass = '',
    children,
    ...rest
  }: Props = $props();

  const base =
    'inline-flex items-center justify-center gap-2 rounded-full px-5 py-2.5 text-sm font-semibold ' +
    'transition-all duration-200 active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-40 ' +
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60';

  const variants: Record<Variant, string> = {
    primary:
      'bg-accent text-white shadow-[var(--shadow-glow)] hover:bg-accent-strong hover:-translate-y-0.5 hover:shadow-lg',
    danger:
      'bg-danger text-white hover:brightness-110 hover:-translate-y-0.5 shadow-[0_8px_20px_-8px_var(--color-danger)]',
    ghost:
      'bg-transparent text-secondary hover:bg-[var(--surface-hover)] hover:text-primary border border-[var(--border-soft)]'
  };
</script>

<button class={`${base} ${variants[variant]} ${klass}`} {...rest}>
  {@render children()}
</button>

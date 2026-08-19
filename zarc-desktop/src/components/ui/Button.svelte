<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Icon, { type IconName } from './Icon.svelte';
  import Spinner from './Spinner.svelte';

  type Variant = 'primary' | 'danger' | 'ghost' | 'subtle';
  type Size = 'sm' | 'md';

  interface Props extends HTMLButtonAttributes {
    variant?: Variant;
    size?: Size;
    icon?: IconName;
    /** 显示转圈并自动禁用；文案保持不变，避免按钮宽度跳动。 */
    loading?: boolean;
    class?: string;
    children: Snippet;
  }

  let {
    variant = 'primary',
    size = 'md',
    icon,
    loading = false,
    disabled = false,
    type = 'button',
    class: klass = '',
    children,
    ...rest
  }: Props = $props();

  const base =
    'inline-flex select-none items-center justify-center gap-2 rounded-control font-semibold ' +
    'transition-[transform,background-color,color,box-shadow,border-color] duration-200 ' +
    'active:scale-[0.98] disabled:pointer-events-none disabled:opacity-45';

  const sizes: Record<Size, string> = {
    sm: 'px-3 py-1.5 text-xs',
    md: 'px-4 py-2.5 text-sm'
  };

  const variants: Record<Variant, string> = {
    primary:
      'bg-accent text-accent-fg shadow-[var(--shadow-glow)] hover:bg-accent-strong hover:-translate-y-px',
    danger:
      'bg-danger text-white shadow-[0_8px_22px_-10px_var(--color-danger)] hover:brightness-110 hover:-translate-y-px',
    ghost:
      'border border-line-strong bg-panel-solid text-fg-soft hover:-translate-y-px hover:border-accent/60 hover:text-fg',
    subtle: 'bg-transparent text-fg-soft hover:bg-inset hover:text-fg'
  };
</script>

<button
  {type}
  class="{base} {sizes[size]} {variants[variant]} {klass}"
  disabled={disabled || loading}
  {...rest}
>
  {#if loading}
    <Spinner size={size === 'sm' ? 12 : 14} />
  {:else if icon}
    <Icon name={icon} size={size === 'sm' ? 14 : 16} />
  {/if}
  {@render children()}
</button>

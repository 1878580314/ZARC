<script lang="ts">
  import Icon, { type IconName } from './Icon.svelte';

  interface Props {
    checked: boolean;
    label: string;
    description?: string;
    icon?: IconName;
    disabled?: boolean;
    /** 打开会带来风险时置为 true（例如「完成后删除源」）。 */
    danger?: boolean;
    onChange?: (checked: boolean) => void;
  }

  let {
    checked = $bindable(),
    label,
    description,
    icon,
    disabled = false,
    danger = false,
    onChange
  }: Props = $props();

  function toggle(): void {
    if (disabled) return;
    checked = !checked;
    onChange?.(checked);
  }

  let onColor = $derived(danger ? 'bg-danger' : 'bg-accent');
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  {disabled}
  onclick={toggle}
  class="flex w-full items-start gap-3 rounded-control px-3 py-2.5 text-left transition-colors hover:bg-inset disabled:pointer-events-none disabled:opacity-45"
>
  <span
    class="relative mt-0.5 h-[1.35rem] w-10 shrink-0 rounded-full transition-colors duration-300 {checked
      ? onColor
      : 'bg-inset-strong'}"
  >
    <span
      class="absolute top-[0.175rem] left-[0.175rem] h-4 w-4 rounded-full bg-white shadow-sm transition-transform duration-300 {checked
        ? 'translate-x-[1.15rem]'
        : ''}"
    ></span>
  </span>

  <span class="flex min-w-0 flex-col gap-0.5">
    <span class="flex items-center gap-1.5 text-sm leading-tight font-medium text-fg">
      {#if icon}
        <Icon name={icon} size={14} class={checked && danger ? 'text-danger' : 'text-fg-faint'} />
      {/if}
      {label}
    </span>
    {#if description}
      <span class="text-xs leading-snug text-fg-faint">{description}</span>
    {/if}
  </span>
</button>

<script lang="ts">
  interface Props {
    checked: boolean;
    label: string;
    description?: string;
    onChange?: (checked: boolean) => void;
  }

  let { checked = $bindable(), label, description, onChange }: Props = $props();

  function toggle() {
    checked = !checked;
    onChange?.(checked);
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  onclick={toggle}
  class="group flex w-full items-center gap-3 rounded-2xl px-3 py-2.5 text-left transition-colors hover:bg-[var(--surface-hover)]"
>
  <span
    class="relative h-6 w-11 shrink-0 rounded-full transition-colors duration-300 {checked
      ? 'bg-accent'
      : 'bg-[var(--border-soft)]'}"
  >
    <span
      class="absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white shadow-md transition-transform duration-300 {checked
        ? 'translate-x-5'
        : ''}"
    ></span>
  </span>
  <span class="flex flex-col">
    <span class="text-sm font-medium text-primary">{label}</span>
    {#if description}
      <span class="text-xs text-muted">{description}</span>
    {/if}
  </span>
</button>

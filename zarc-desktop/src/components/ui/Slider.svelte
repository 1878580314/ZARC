<script lang="ts">
  interface Props {
    value: number;
    min: number;
    max: number;
    step?: number;
    label?: string;
    onChange?: (value: number) => void;
  }

  let { value = $bindable(), min, max, step = 1, label, onChange }: Props = $props();

  function handle(e: Event) {
    const target = e.target as HTMLInputElement;
    value = Number(target.value);
    onChange?.(value);
  }
</script>

<div class="flex flex-col gap-2">
  {#if label}
    <div class="flex items-center justify-between text-sm">
      <span class="text-secondary font-medium">{label}</span>
      <span
        class="min-w-[2.5rem] rounded-full bg-accent/15 px-2.5 py-0.5 text-center font-semibold text-accent"
      >
        {value}
      </span>
    </div>
  {/if}
  <input
    type="range"
    {min}
    {max}
    {step}
    {value}
    oninput={handle}
    class="zarc-range h-2 w-full cursor-pointer appearance-none rounded-full bg-[var(--border-soft)]"
  />
</div>

<style>
  .zarc-range::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--color-accent);
    border: 3px solid var(--surface-solid);
    box-shadow: 0 0 0 1px var(--color-accent), 0 4px 10px -2px var(--color-accent);
    transition: transform 0.15s ease;
  }
  .zarc-range::-webkit-slider-thumb:hover {
    transform: scale(1.18);
  }
  .zarc-range::-moz-range-thumb {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--color-accent);
    border: 3px solid var(--surface-solid);
    box-shadow: 0 0 0 1px var(--color-accent);
    cursor: pointer;
  }
</style>

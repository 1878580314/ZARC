<script lang="ts">
  interface Props {
    value: number;
    min: number;
    max: number;
    step?: number;
    disabled?: boolean;
    /** 刻度标记，形如 `[{ at: 3, label: '快' }]`。 */
    marks?: { at: number; label: string }[];
    ariaLabel?: string;
    id?: string;
  }

  let {
    value = $bindable(),
    min,
    max,
    step = 1,
    disabled = false,
    marks = [],
    ariaLabel,
    id
  }: Props = $props();

  // 已填充比例。原实现只有一条灰轨，看不出当前位置在整个量程里的相对深浅。
  let fill = $derived(((value - min) / (max - min)) * 100);
</script>

<div class="flex flex-col gap-2">
  <input
    {id}
    type="range"
    {min}
    {max}
    {step}
    {value}
    {disabled}
    aria-label={ariaLabel}
    oninput={(e) => (value = Number(e.currentTarget.value))}
    class="zarc-range"
    style="--fill: {fill}%"
  />

  {#if marks.length > 0}
    <div class="relative h-4 text-[0.65rem] text-fg-faint">
      {#each marks as mark (mark.at)}
        {@const offset = ((mark.at - min) / (max - min)) * 100}
        <span
          class="absolute -translate-x-1/2 whitespace-nowrap transition-colors {value === mark.at
            ? 'font-semibold text-accent'
            : ''}"
          style="left: clamp(1.25rem, {offset}%, calc(100% - 1.25rem))"
        >
          {mark.label}
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .zarc-range {
    width: 100%;
    height: 1.25rem;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }
  .zarc-range:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  /* 轨道：已填充部分用主色，剩余部分用内陷色。 */
  .zarc-range::-webkit-slider-runnable-track {
    height: 0.375rem;
    border-radius: var(--radius-pill);
    background: linear-gradient(
      to right,
      var(--zarc-accent) 0%,
      var(--zarc-accent) var(--fill),
      var(--zarc-inset-strong) var(--fill),
      var(--zarc-inset-strong) 100%
    );
  }
  .zarc-range::-moz-range-track {
    height: 0.375rem;
    border-radius: var(--radius-pill);
    background: var(--zarc-inset-strong);
  }
  .zarc-range::-moz-range-progress {
    height: 0.375rem;
    border-radius: var(--radius-pill);
    background: var(--zarc-accent);
  }

  .zarc-range::-webkit-slider-thumb {
    appearance: none;
    width: 1.125rem;
    height: 1.125rem;
    margin-top: -0.375rem;
    border-radius: 50%;
    background: var(--zarc-panel-solid);
    border: 3px solid var(--zarc-accent);
    box-shadow: var(--zarc-shadow-soft);
    transition:
      transform 0.15s ease,
      box-shadow 0.15s ease;
  }
  .zarc-range:hover:not(:disabled)::-webkit-slider-thumb {
    transform: scale(1.15);
  }
  .zarc-range:active:not(:disabled)::-webkit-slider-thumb {
    transform: scale(1.05);
    box-shadow: 0 0 0 6px var(--zarc-accent-wash);
  }

  .zarc-range::-moz-range-thumb {
    width: 1.125rem;
    height: 1.125rem;
    border-radius: 50%;
    background: var(--zarc-panel-solid);
    border: 3px solid var(--zarc-accent);
    box-shadow: var(--zarc-shadow-soft);
  }
</style>

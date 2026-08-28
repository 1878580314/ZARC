<script lang="ts">
  import { clamp } from '../../lib/format';
  import { t } from '../../lib/i18n/index.svelte';

  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    suffix?: string;
    disabled?: boolean;
    id?: string;
  }

  let {
    value = $bindable(0),
    min = 0,
    max = Number.MAX_SAFE_INTEGER,
    step = 1,
    suffix,
    disabled = false,
    id
  }: Props = $props();

  /**
   * The raw text inside the input.
   *
   * Don't `bind:value` directly to a number: the moment the user clears the
   * field it would be written back to 0, leaving an undeletable zero after the
   * cursor. Here the text is edited freely and normalized only on blur.
   */
  let draft = $state(String(value));
  let editing = $state(false);

  // Sync external value changes (drag-and-drop, presets, SFX mode) back into the field.
  $effect(() => {
    if (!editing) {
      draft = String(value);
    }
  });

  function commit(): void {
    editing = false;
    const parsed = Number.parseInt(draft, 10);
    // min/max used to be passed straight through to <input>, and the browser doesn't
    // stop programmatic out-of-range values, so "minimum level 999" went to the backend
    // as-is. Clamp for real here.
    value = Number.isFinite(parsed) ? clamp(parsed, min, max) : min;
    draft = String(value);
  }

  function nudge(delta: number): void {
    value = clamp(value + delta, min, max);
    draft = String(value);
  }
</script>

<div class="relative flex items-stretch">
  <input
    {id}
    type="text"
    inputmode="numeric"
    value={draft}
    {disabled}
    oninput={(e) => {
      editing = true;
      draft = e.currentTarget.value;
    }}
    onblur={commit}
    onkeydown={(e) => {
      if (e.key === 'Enter') {
        commit();
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        nudge(step);
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        nudge(-step);
      }
    }}
    class="control pr-20 tabular-nums {suffix ? 'pr-[5.5rem]' : ''}"
  />

  <div class="pointer-events-none absolute inset-y-0 right-2 flex items-center gap-1">
    {#if suffix}
      <span class="mr-1 text-xs text-fg-faint">{suffix}</span>
    {/if}
    <button
      type="button"
      class="pointer-events-auto flex h-6 w-6 items-center justify-center rounded-md text-sm font-bold text-fg-faint transition-colors hover:bg-inset hover:text-fg disabled:opacity-30 disabled:hover:bg-transparent"
      onclick={() => nudge(-step)}
      disabled={disabled || value <= min}
      aria-label={t('ui.decrease')}
      tabindex="-1"
    >
      −
    </button>
    <button
      type="button"
      class="pointer-events-auto flex h-6 w-6 items-center justify-center rounded-md text-sm font-bold text-fg-faint transition-colors hover:bg-inset hover:text-fg disabled:opacity-30 disabled:hover:bg-transparent"
      onclick={() => nudge(step)}
      disabled={disabled || value >= max}
      aria-label={t('ui.increase')}
      tabindex="-1"
    >
      +
    </button>
  </div>
</div>

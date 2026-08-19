<script lang="ts">
  import { clamp } from '../../lib/format';

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
   * 输入框里的原始文本。
   *
   * 不能直接 `bind:value` 到数字：那样一旦用户删空内容就立刻被写回 0，
   * 光标后面还跟着一个删不掉的零。这里让文本自由编辑，失焦时才归一化。
   */
  let draft = $state(String(value));
  let editing = $state(false);

  // 外部（拖放、预设、SFX 模式）改动 value 时同步回文本框。
  $effect(() => {
    if (!editing) {
      draft = String(value);
    }
  });

  function commit(): void {
    editing = false;
    const parsed = Number.parseInt(draft, 10);
    // min/max 过去只是透传给 <input>，浏览器并不阻止程序化的越界值，
    // 于是「最低等级 999」会原样发给后端。这里真正夹紧。
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
      aria-label="减少"
      tabindex="-1"
    >
      −
    </button>
    <button
      type="button"
      class="pointer-events-auto flex h-6 w-6 items-center justify-center rounded-md text-sm font-bold text-fg-faint transition-colors hover:bg-inset hover:text-fg disabled:opacity-30 disabled:hover:bg-transparent"
      onclick={() => nudge(step)}
      disabled={disabled || value >= max}
      aria-label="增加"
      tabindex="-1"
    >
      +
    </button>
  </div>
</div>

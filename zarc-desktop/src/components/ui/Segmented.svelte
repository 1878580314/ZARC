<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  // 类型必须挂在 module 脚本上：带 generics 的实例脚本不允许 export。
  export interface SegmentOption<V extends string> {
    value: V;
    label: string;
    icon?: IconName;
    hint?: string;
  }
</script>

<script lang="ts" generics="T extends string">
  import Icon from './Icon.svelte';

  interface Props {
    value: T;
    options: SegmentOption<T>[];
    disabled?: boolean;
    /** 无障碍名称，屏幕阅读器用它描述这组按钮的用途。 */
    ariaLabel?: string;
  }

  let { value = $bindable(), options, disabled = false, ariaLabel }: Props = $props();

  /**
   * 两三个互斥选项用分段控件比原生 `<select>` 好：全部选项一眼可见，
   * 一次点击就能切换，也不会弹出跟应用外观完全不搭的系统下拉框。
   */
  function move(delta: number): void {
    const index = options.findIndex((o) => o.value === value);
    const next = (index + delta + options.length) % options.length;
    value = options[next].value;
  }
</script>

<!-- 焦点落在选中的那个 radio 上（roving tabindex），容器本身不进 Tab 序列。 -->
<div
  role="radiogroup"
  tabindex={-1}
  aria-label={ariaLabel}
  class="flex gap-1 rounded-control border border-line bg-inset p-1"
  onkeydown={(e) => {
    if (disabled) return;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      move(1);
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      move(-1);
    }
  }}
>
  {#each options as option (option.value)}
    {@const active = option.value === value}
    <button
      type="button"
      role="radio"
      aria-checked={active}
      title={option.hint}
      {disabled}
      tabindex={active ? 0 : -1}
      onclick={() => (value = option.value)}
      class="flex flex-1 items-center justify-center gap-2 rounded-[calc(var(--radius-control)-0.25rem)] px-3 py-2 text-xs font-semibold whitespace-nowrap transition-all duration-200 disabled:pointer-events-none disabled:opacity-45 {active
        ? 'bg-panel-solid text-accent shadow-[var(--shadow-soft)]'
        : 'text-fg-faint hover:text-fg'}"
    >
      {#if option.icon}
        <Icon name={option.icon} size={15} />
      {/if}
      {option.label}
    </button>
  {/each}
</div>

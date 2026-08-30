<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  // 类型必须放在 module script：带泛型的实例脚本无法导出。 / The type must live on the module script: instance scripts with generics cannot export.
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
    /** 无障碍名称；屏幕阅读器用它描述按钮组的用途。 / Accessible name; screen readers use it to describe what this button group is for. */
    ariaLabel?: string;
  }

  let { value = $bindable(), options, disabled = false, ariaLabel }: Props = $props();

  /**
   * 两三个互斥选项时分段控件优于原生 `<select>`：所有选项一目了然、单击即切换，
   * 且没有与应用外观冲突的系统下拉弹层。
   * For two or three exclusive options a segmented control beats a native
   * `<select>`: every option is visible at a glance, one click switches, and
   * there's no system dropdown popup that clashes with the app's look.
   */
  function move(delta: number): void {
    const index = options.findIndex((o) => o.value === value);
    const next = (index + delta + options.length) % options.length;
    value = options[next].value;
  }
</script>

<!-- 焦点落在选中的单选项上（roving tabindex）；容器本身不进入 Tab 序。 / Focus lands on the selected radio (roving tabindex); the container itself stays out of the tab order. -->
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

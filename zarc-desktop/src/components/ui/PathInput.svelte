<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';

  interface Props {
    value: string;
    placeholder?: string;
    disabled?: boolean;
    /** 只读用于「宿主由后端决定」的场景（自解压模式），普通场景务必留空。 */
    readonly?: boolean;
    invalid?: boolean;
    icon?: IconName;
    /**
     * 手动输入定稿时回调（失焦、回车、清空）。
     *
     * 路径写进 store 会触发一次目录统计，逐字符回调等于每敲一个键就走一遍
     * 文件树；因此只在用户"输完了"的时刻推送。
     */
    onCommit?: (value: string) => void;
    /** 右侧按钮组，如「文件」「目录」「选择」。 */
    actions?: Snippet;
  }

  let {
    value = $bindable(''),
    placeholder = '',
    disabled = false,
    readonly = false,
    invalid = false,
    icon = 'file',
    onCommit,
    actions
  }: Props = $props();

  let canClear = $derived(value.length > 0 && !disabled && !readonly);

  function clear(): void {
    value = '';
    onCommit?.('');
  }
</script>

<div class="flex items-stretch gap-2">
  <div class="relative min-w-0 flex-1">
    <span
      class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 {invalid
        ? 'text-danger'
        : 'text-fg-faint'}"
    >
      <Icon name={icon} size={16} />
    </span>
    <!--
      这里过去同时写了 `bind:value` 和 `readonly`，于是「留空则自动生成」的
      输出路径根本敲不进去，只能靠文件对话框。现在只有明确传入 readonly 才只读。
    -->
    <input
      type="text"
      bind:value
      {placeholder}
      {disabled}
      {readonly}
      spellcheck="false"
      autocomplete="off"
      aria-invalid={invalid}
      onblur={() => onCommit?.(value)}
      onkeydown={(e) => {
        if (e.key === 'Enter') onCommit?.(value);
      }}
      class="control pl-9 {canClear ? 'pr-9' : ''} truncate"
    />
    {#if canClear}
      <button
        type="button"
        onclick={clear}
        aria-label="清空"
        title="清空"
        class="absolute top-1/2 right-2 -translate-y-1/2 rounded-md p-1 text-fg-faint transition-colors hover:bg-inset hover:text-fg"
      >
        <Icon name="close" size={14} />
      </button>
    {/if}
  </div>

  {#if actions}
    {@render actions()}
  {/if}
</div>

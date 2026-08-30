<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import { t } from '../../lib/i18n/index.svelte';

  interface Props {
    value: string;
    placeholder?: string;
    disabled?: boolean;
    /** 只读用于「后端决定宿主」的场景（自解压模式）；常规场景保持关闭。 / Read-only is for the "backend decides the host" case (self-extracting mode); leave it off in normal scenarios. */
    readonly?: boolean;
    invalid?: boolean;
    icon?: IconName;
    /**
     * 手动输入敲定（失焦、回车、清空）时调用。
     * Called when manual input is finalized (blur, Enter, clear).
     *
     * 路径写入 store 会触发目录扫描；按键即回调会每敲一键就遍历文件树，
     * 因此只在用户「输完」的时刻推送。
     * Writing the path into the store triggers a directory scan; a per-keystroke
     * callback would walk the file tree on every key pressed, so only push at
     * the moment the user is "done typing".
     */
    onCommit?: (value: string) => void;
    /** 右侧按钮组，如「文件」「文件夹」「浏览」。 / Right-side button group, e.g. "File", "Folder", "Browse". */
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
      此处曾同时设置 `bind:value` 和 `readonly`，导致「留空自动生成」的输出路径
      完全无法手动输入，只能靠文件对话框。现在仅在显式传入 readonly 时才只读。
      This used to set both `bind:value` and `readonly`, so the "auto-generate
      when empty" output path could not be typed into at all and only the file
      dialog worked. Now it is read-only only when readonly is explicitly passed.
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
        aria-label={t('ui.clear')}
        title={t('ui.clear')}
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

<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import { t } from '../../lib/i18n/index.svelte';

  interface Props {
    value: string;
    placeholder?: string;
    disabled?: boolean;
    /** Read-only is for the "backend decides the host" case (self-extracting mode); leave it off in normal scenarios. */
    readonly?: boolean;
    invalid?: boolean;
    icon?: IconName;
    /**
     * Called when manual input is finalized (blur, Enter, clear).
     *
     * Writing the path into the store triggers a directory scan; a per-keystroke
     * callback would walk the file tree on every key pressed, so only push at
     * the moment the user is "done typing".
     */
    onCommit?: (value: string) => void;
    /** Right-side button group, e.g. "File", "Folder", "Browse". */
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

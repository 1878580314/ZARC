<script lang="ts">
  import Icon from './Icon.svelte';
  import { passwordStrength } from '../../lib/format';
  import { t } from '../../lib/i18n/index.svelte';

  interface Props {
    value: string;
    placeholder?: string;
    disabled?: boolean;
    /** Whether to show the strength bar; when extracting, the password already exists, so rating its strength is pointless. */
    showStrength?: boolean;
    id?: string;
  }

  let {
    value = $bindable(''),
    placeholder = '',
    disabled = false,
    showStrength = false,
    id
  }: Props = $props();

  let revealed = $state(false);
  let strength = $derived(passwordStrength(value));

  const barColors = [
    'bg-danger',
    'bg-danger',
    'bg-warning',
    'bg-success',
    'bg-success'
  ];
  const textColors = [
    'text-danger',
    'text-danger',
    'text-warning',
    'text-success',
    'text-success'
  ];
</script>

<div class="flex flex-col gap-2">
  <div class="relative">
    <span class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-fg-faint">
      <Icon name="shield" size={16} />
    </span>
    <input
      {id}
      type={revealed ? 'text' : 'password'}
      bind:value
      {placeholder}
      {disabled}
      spellcheck="false"
      autocomplete="off"
      autocapitalize="off"
      class="control pr-10 pl-9"
    />
    <button
      type="button"
      onclick={() => (revealed = !revealed)}
      aria-label={revealed ? t('ui.hidePassword') : t('ui.showPassword')}
      title={revealed ? t('ui.hidePassword') : t('ui.showPassword')}
      class="absolute top-1/2 right-2 -translate-y-1/2 rounded-md p-1.5 text-fg-faint transition-colors hover:bg-inset hover:text-fg"
    >
      <Icon name={revealed ? 'eyeOff' : 'eye'} size={15} />
    </button>
  </div>

  {#if showStrength && value.length > 0}
    <div class="flex items-center gap-2.5">
      <div class="flex flex-1 gap-1">
        {#each [0, 1, 2, 3] as index (index)}
          <span
            class="h-1 flex-1 rounded-full transition-colors duration-300 {index < strength.score
              ? barColors[strength.score]
              : 'bg-inset-strong'}"
          ></span>
        {/each}
      </div>
      <span class="w-8 shrink-0 text-right text-xs font-semibold {textColors[strength.score]}">
        {strength.label}
      </span>
    </div>
    <p class="text-xs text-fg-faint">{strength.hint}</p>
  {/if}
</div>

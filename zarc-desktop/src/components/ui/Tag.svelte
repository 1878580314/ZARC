<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';

  type Tone = 'accent' | 'neutral' | 'success' | 'warning' | 'danger';

  interface Props {
    tone?: Tone;
    icon?: IconName;
    children: Snippet;
  }

  let { tone = 'accent', icon, children }: Props = $props();

  const tones: Record<Tone, string> = {
    accent: 'bg-accent-wash text-accent',
    neutral: 'bg-inset text-fg-soft',
    success: 'bg-success/12 text-success',
    warning: 'bg-warning/15 text-warning',
    danger: 'bg-danger/12 text-danger'
  };
</script>

<span
  class="inline-flex shrink-0 items-center gap-1.5 rounded-pill px-2.5 py-1 text-xs font-semibold whitespace-nowrap {tones[
    tone
  ]}"
>
  {#if icon}
    <Icon name={icon} size={13} />
  {/if}
  {@render children()}
</span>

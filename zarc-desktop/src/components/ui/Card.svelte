<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';

  interface Props {
    title?: string;
    subtitle?: string;
    icon?: IconName;
    /** 标题行右侧的操作区（按钮、标签等）。 / Action area on the right of the header row (buttons, tags, etc.). */
    actions?: Snippet;
    class?: string;
    children: Snippet;
  }

  let { title, subtitle, icon, actions, class: klass = '', children }: Props = $props();

  let hasHeader = $derived(Boolean(title || actions));
</script>

<section class="panel rounded-panel {klass}">
  {#if hasHeader}
    <header class="flex items-start justify-between gap-4 border-b border-line px-5 py-4">
      <div class="flex min-w-0 items-center gap-3">
        {#if icon}
          <span
            class="flex h-9 w-9 shrink-0 items-center justify-center rounded-control bg-accent-wash text-accent"
          >
            <Icon name={icon} size={18} />
          </span>
        {/if}
        <div class="min-w-0">
          {#if title}
            <h2 class="truncate text-sm font-bold tracking-tight text-fg">{title}</h2>
          {/if}
          {#if subtitle}
            <p class="mt-0.5 truncate text-xs text-fg-faint">{subtitle}</p>
          {/if}
        </div>
      </div>
      {#if actions}
        <div class="flex shrink-0 items-center gap-2">{@render actions()}</div>
      {/if}
    </header>
  {/if}

  <div class="p-5">
    {@render children()}
  </div>
</section>

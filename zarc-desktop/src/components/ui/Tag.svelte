<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

<span
  class="inline-flex items-center gap-1.5 rounded-full bg-accent/12 px-3 py-1 text-xs font-semibold text-accent"
>
  {@render children()}
</span>

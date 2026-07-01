<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    class?: string;
    children: Snippet;
  }

  let { class: klass = '', children }: Props = $props();
</script>

<section class="glass rounded-[var(--radius-card)] p-6 {klass}">
  {@render children()}
</section>

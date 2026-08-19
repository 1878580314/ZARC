<script lang="ts">
  interface Props {
    size?: number;
    class?: string;
  }

  let { size = 16, class: klass = '' }: Props = $props();
</script>

<span
  class="inline-block shrink-0 animate-[var(--animate-spin-slow)] rounded-full border-2 border-current border-t-transparent {klass}"
  style="width: {size}px; height: {size}px"
  aria-hidden="true"
></span>

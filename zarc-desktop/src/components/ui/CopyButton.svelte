<script lang="ts">
  import Icon from './Icon.svelte';
  import { toasts } from '../../stores/toast.svelte';

  interface Props {
    text: string;
    label?: string;
    class?: string;
  }

  let { text, label = '复制', class: klass = '' }: Props = $props();

  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      clearTimeout(timer);
      timer = setTimeout(() => (copied = false), 1600);
    } catch {
      // WebView 未授予剪贴板权限时给出可见反馈，而不是静默失败。
      toasts.warn('复制失败', '当前环境不允许写入剪贴板，请手动选中文本复制。');
    }
  }

  $effect(() => () => clearTimeout(timer));
</script>

<button
  type="button"
  onclick={copy}
  aria-label={label}
  title={copied ? '已复制' : label}
  class="rounded-md p-1 transition-colors {copied
    ? 'text-success'
    : 'text-fg-faint hover:bg-inset hover:text-fg'} {klass}"
>
  <Icon name={copied ? 'check' : 'copy'} size={14} />
</button>

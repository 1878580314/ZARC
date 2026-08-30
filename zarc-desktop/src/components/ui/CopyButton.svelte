<script lang="ts">
  import Icon from './Icon.svelte';
  import { toasts } from '../../stores/toast.svelte';
  import { t } from '../../lib/i18n/index.svelte';

  interface Props {
    text: string;
    label?: string;
    class?: string;
  }

  let { text, label = t('ui.copy'), class: klass = '' }: Props = $props();

  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      clearTimeout(timer);
      timer = setTimeout(() => (copied = false), 1600);
    } catch {
      // WebView 拒绝剪贴板访问时给出可见反馈，而不是静默失败。 / Give visible feedback when the WebView denies clipboard access, instead of failing silently.
      toasts.warn(t('ui.copyFailed'), t('ui.copyFailedDetail'));
    }
  }

  $effect(() => () => clearTimeout(timer));
</script>

<button
  type="button"
  onclick={copy}
  aria-label={label}
  title={copied ? t('ui.copied') : label}
  class="rounded-md p-1 transition-colors {copied
    ? 'text-success'
    : 'text-fg-faint hover:bg-inset hover:text-fg'} {klass}"
>
  <Icon name={copied ? 'check' : 'copy'} size={14} />
</button>

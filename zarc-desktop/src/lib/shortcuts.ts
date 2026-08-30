import type { ViewId } from './api';
import { app } from '../stores/app.svelte';
import { task } from '../stores/task.svelte';
import { theme } from '../stores/theme.svelte';

export interface ShortcutHint {
  /** 按键序列，每个 <kbd> 渲染一项。 / Key sequence, rendered one item per <kbd>. */
  keys: string[];
  /** i18n 词典键；渲染时经 t() 解析，因此条目会随语言切换。 / i18n dictionary key; resolved via t() at render time so entries follow locale switches. */
  descriptionKey: string;
}

export const SHORTCUTS: ShortcutHint[] = [
  { keys: ['Ctrl', '1 / 2 / 3'], descriptionKey: 'store.sc.switchViews' },
  { keys: ['Ctrl', 'Enter'], descriptionKey: 'store.sc.runAction' },
  { keys: ['Esc'], descriptionKey: 'store.sc.esc' },
  { keys: ['Ctrl', 'D'], descriptionKey: 'store.sc.theme' },
  { keys: ['Ctrl', '/'], descriptionKey: 'store.sc.openPanel' }
];

const VIEW_BY_DIGIT: Record<string, ViewId> = {
  '1': 'compress',
  '2': 'decompress',
  '3': 'benchmark'
};

/** 每个视图注册其主操作，通过 Ctrl+Enter 触发。 / Each view registers its primary action, invoked on Ctrl+Enter. */
const primaryActions = new Map<ViewId, () => void>();

export function registerPrimaryAction(view: ViewId, action: () => void): () => void {
  primaryActions.set(view, action);
  return () => {
    if (primaryActions.get(view) === action) {
      primaryActions.delete(view);
    }
  };
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable
  );
}

export function initShortcuts(): () => void {
  function onKeydown(event: KeyboardEvent): void {
    // Esc 即使在文本框内也必须生效——那正是最需要它的时候。
    // Esc must work even inside text fields — that is when it is needed most.
    if (event.key === 'Escape') {
      if (app.shortcutsOpen) {
        event.preventDefault();
        app.shortcutsOpen = false;
      } else if (task.busy && !task.aborting) {
        event.preventDefault();
        task.requestAbort();
      }
      return;
    }

    const mod = event.ctrlKey || event.metaKey;
    if (!mod) return;

    if (event.key === 'Enter') {
      // 在文本框内提交是常见的表单惯例。
      // Submitting from inside a text field is the usual form convention.
      const action = primaryActions.get(app.currentView);
      if (action) {
        event.preventDefault();
        action();
      }
      return;
    }

    // 在文本框内不拦截其他组合键，以保留 Ctrl+A/C/V。
    // Don't intercept other combos inside text fields, to keep Ctrl+A/C/V intact.
    if (isTypingTarget(event.target)) return;

    const view = VIEW_BY_DIGIT[event.key];
    if (view) {
      event.preventDefault();
      if (!app.isSfx) app.setView(view);
      return;
    }

    if (event.key.toLowerCase() === 'd') {
      event.preventDefault();
      theme.toggle();
      return;
    }

    if (event.key === '/') {
      event.preventDefault();
      app.shortcutsOpen = !app.shortcutsOpen;
    }
  }

  window.addEventListener('keydown', onKeydown);
  return () => window.removeEventListener('keydown', onKeydown);
}

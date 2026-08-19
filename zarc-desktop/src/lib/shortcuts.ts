import type { ViewId } from './api';
import { app } from '../stores/app.svelte';
import { task } from '../stores/task.svelte';
import { theme } from '../stores/theme.svelte';

export interface ShortcutHint {
  /** 按键序列，逐个渲染成 <kbd>。 */
  keys: string[];
  description: string;
}

export const SHORTCUTS: ShortcutHint[] = [
  { keys: ['Ctrl', '1 / 2 / 3'], description: '切换压缩 / 解压 / 测试' },
  { keys: ['Ctrl', 'Enter'], description: '执行当前视图的主操作' },
  { keys: ['Esc'], description: '中止运行中的任务 / 关闭面板' },
  { keys: ['Ctrl', 'D'], description: '切换深色 / 浅色主题' },
  { keys: ['Ctrl', '/'], description: '打开这个面板' }
];

const VIEW_BY_DIGIT: Record<string, ViewId> = {
  '1': 'compress',
  '2': 'decompress',
  '3': 'benchmark'
};

/** 各视图注册自己的主操作，Ctrl+Enter 时调用。 */
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
    // Esc 即使在输入框里也应当有效——那是最需要它的时刻。
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
      // 输入框内也允许提交，这是表单的通行约定。
      const action = primaryActions.get(app.currentView);
      if (action) {
        event.preventDefault();
        action();
      }
      return;
    }

    // 其余组合键在输入框内不拦截，避免抢走 Ctrl+A/C/V。
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

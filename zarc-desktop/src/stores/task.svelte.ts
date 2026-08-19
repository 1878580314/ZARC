import type { ProgressKind } from '../lib/api';
import { api } from '../lib/api';
import { app } from './app.svelte';
import { progress } from './progress.svelte';
import { toasts } from './toast.svelte';
import { normalizeError } from '../lib/format';

/** 抛出它可以跳过默认的错误吐司（调用方已自行提示）。 */
export class SilentTaskError extends Error {}

class TaskStore {
  busy = $state(false);
  activeKind = $state<ProgressKind | null>(null);
  aborting = $state(false);

  /** 用户主动中止：这不是错误，提示语气应当不同。 */
  #abortRequested = false;

  async run(kind: ProgressKind, statusText: string, fn: () => Promise<void>): Promise<boolean> {
    if (this.busy) {
      toasts.warn('已有任务在运行', '请等待当前任务结束，或先点击「停止」。');
      return false;
    }

    this.busy = true;
    this.activeKind = kind;
    this.aborting = false;
    this.#abortRequested = false;
    app.setStatus(statusText, 'busy');
    progress.reset(kind, statusText);

    try {
      await fn();
      progress.succeed(kind);
      return true;
    } catch (error) {
      const message = normalizeError(error);
      progress.fail(kind, message);
      if (this.#abortRequested) {
        app.setStatus('任务已中止。', 'idle');
        toasts.info('任务已中止');
      } else {
        app.setStatus(message, 'error');
        if (!(error instanceof SilentTaskError)) {
          toasts.error('任务失败', message);
        }
      }
      return false;
    } finally {
      this.busy = false;
      this.activeKind = null;
      this.aborting = false;
      this.#abortRequested = false;
      // 基准测试没有进度条槽位；压缩/解压保留终态卡片，由下一次任务或用户清除。
      if (kind === 'benchmark') {
        progress.hide(kind);
      }
    }
  }

  requestAbort(): void {
    if (!this.busy || this.aborting) return;
    this.#abortRequested = true;
    this.aborting = true;
    void api.abort();
    app.setStatus('正在停止任务...', 'busy');
  }
}

export const task = new TaskStore();

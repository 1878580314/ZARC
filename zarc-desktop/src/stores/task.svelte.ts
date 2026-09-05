import type { ProgressKind } from '../lib/api';
import { api } from '../lib/api';
import { app } from './app.svelte';
import { progress } from './progress.svelte';
import { toasts } from './toast.svelte';
import { normalizeError } from '../lib/format';
import { t } from '../lib/i18n/index.svelte';

class TaskStore {
  activeKind = $state<ProgressKind | null>(null);

  /** 用户主动中止：不算错误，措辞应有所不同。 / User-initiated abort: not an error, so the messaging should sound different. */
  #abortRequested = $state(false);

  /** 派生状态：busy ⇔ 有活动任务，aborting ⇔ 已请求中止，不再手动同步。
   *  Derived: busy ⇔ a task is active, aborting ⇔ abort was requested — never synced by hand. */
  get busy(): boolean {
    return this.activeKind !== null;
  }

  get aborting(): boolean {
    return this.#abortRequested;
  }

  async run(kind: ProgressKind, statusText: string, fn: () => Promise<void>): Promise<boolean> {
    if (this.busy) {
      toasts.warn(t('toast.onePathOnly'), t('store.busyHint'));
      return false;
    }

    this.activeKind = kind;
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
        app.setStatus(t('status.taskAborted'), 'idle');
        toasts.info(t('toast.taskAborted'));
      } else {
        app.setStatus(message, 'error');
        toasts.error(t('taskFailed'), message);
      }
      return false;
    } finally {
      this.activeKind = null;
      this.#abortRequested = false;
      // 基准测试没有进度槽；压缩/解压保留最终状态卡片，直到下一个任务开始或用户关闭。
      // Benchmark has no progress slot; compress/extract keep their final-state card
      // until the next task starts or the user dismisses it.
      if (kind === 'benchmark') {
        progress.hide(kind);
      }
    }
  }

  requestAbort(): void {
    if (!this.busy || this.aborting) return;
    this.#abortRequested = true;
    void api.abort();
    app.setStatus(t('status.stoppingTask'), 'busy');
  }
}

export const task = new TaskStore();

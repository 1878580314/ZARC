import type { ProgressKind } from '../lib/api';
import { api } from '../lib/api';
import { app } from './app.svelte';
import { progress } from './progress.svelte';
import { toasts } from './toast.svelte';
import { normalizeError } from '../lib/format';
import { t } from '../lib/i18n/index.svelte';

/** Throw this to skip the default error toast (the caller has already surfaced the message). */
export class SilentTaskError extends Error {}

class TaskStore {
  busy = $state(false);
  activeKind = $state<ProgressKind | null>(null);
  aborting = $state(false);

  /** User-initiated abort: not an error, so the messaging should sound different. */
  #abortRequested = false;

  async run(kind: ProgressKind, statusText: string, fn: () => Promise<void>): Promise<boolean> {
    if (this.busy) {
      toasts.warn(t('toast.onePathOnly'), t('store.busyHint'));
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
        app.setStatus(t('status.taskAborted'), 'idle');
        toasts.info(t('toast.taskAborted'));
      } else {
        app.setStatus(message, 'error');
        if (!(error instanceof SilentTaskError)) {
          toasts.error(t('taskFailed'), message);
        }
      }
      return false;
    } finally {
      this.busy = false;
      this.activeKind = null;
      this.aborting = false;
      this.#abortRequested = false;
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
    this.aborting = true;
    void api.abort();
    app.setStatus(t('status.stoppingTask'), 'busy');
  }
}

export const task = new TaskStore();

import type { ProgressKind } from '../lib/api';
import { api } from '../lib/api';
import { app } from './app.svelte';
import { progress } from './progress.svelte';
import { normalizeError } from '../lib/format';

class TaskStore {
  busy = $state(false);
  activeKind = $state<ProgressKind | null>(null);
  aborting = $state(false);

  async run(kind: ProgressKind, statusText: string, fn: () => Promise<void>): Promise<void> {
    this.busy = true;
    this.activeKind = kind;
    this.aborting = false;
    app.setStatus(statusText, 'busy');
    progress.reset(kind);
    try {
      await fn();
    } catch (error) {
      app.setStatus(normalizeError(error), 'error');
    } finally {
      this.busy = false;
      this.activeKind = null;
      this.aborting = false;
      if (kind === 'benchmark') {
        progress.hide(kind);
      }
    }
  }

  requestAbort(): void {
    void api.abort();
    this.aborting = true;
  }
}

export const task = new TaskStore();

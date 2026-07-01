import type { ProgressKind, ProgressPayload } from '../lib/api';

export interface TaskProgress {
  percent: number;
  processedBytes: number;
  totalBytes: number;
  throughputMiBs: number;
  etaSeconds: number | null;
  done: boolean;
  error: string | null;
  visible: boolean;
}

function emptyProgress(): TaskProgress {
  return {
    percent: 0,
    processedBytes: 0,
    totalBytes: 0,
    throughputMiBs: 0,
    etaSeconds: null,
    done: false,
    error: null,
    visible: false
  };
}

class ProgressStore {
  compress = $state<TaskProgress>(emptyProgress());
  decompress = $state<TaskProgress>(emptyProgress());

  update(payload: ProgressPayload): void {
    // Benchmark does not emit progress; the benchmark card is driven by task store.
    if (payload.operation === 'benchmark') return;

    const slot = payload.operation === 'compress' ? this.compress : this.decompress;
    slot.percent = Math.max(0, Math.min(payload.percent, 100));
    slot.processedBytes = payload.processedBytes;
    slot.totalBytes = payload.totalBytes;
    slot.throughputMiBs = payload.throughputMiBs;
    slot.etaSeconds = payload.etaSeconds;
    slot.done = payload.done;
    slot.error = payload.error;
    slot.visible = true;
  }

  reset(kind: ProgressKind): void {
    if (kind === 'benchmark') return;
    this[kind] = { ...emptyProgress(), visible: true };
  }

  hide(kind: ProgressKind): void {
    if (kind === 'benchmark') return;
    this[kind].visible = false;
  }
}

export const progress = new ProgressStore();

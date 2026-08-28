import type { ProgressKind, ProgressPayload } from '../lib/api';
import { translateBackendText } from '../lib/i18n/backend';

export interface TaskProgress {
  percent: number;
  processedBytes: number;
  totalBytes: number;
  throughputMiBs: number;
  etaSeconds: number | null;
  done: boolean;
  error: string | null;
  visible: boolean;
  /** True once at least one backend event has arrived; before that the bar runs in the indeterminate "preparing" state. */
  started: boolean;
  /** Task label, letting the Task Hub tell "compressing" apart from "reading the archive listing". */
  label: string;
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
    visible: false,
    started: false,
    label: ''
  };
}

/** The backend only emits progress events for compress/decompress; benchmark is driven by the task store. */
type TrackedKind = Exclude<ProgressKind, 'benchmark'>;

function isTracked(kind: ProgressKind): kind is TrackedKind {
  return kind === 'compress' || kind === 'decompress';
}

class ProgressStore {
  compress = $state<TaskProgress>(emptyProgress());
  decompress = $state<TaskProgress>(emptyProgress());

  update(payload: ProgressPayload): void {
    if (!isTracked(payload.operation)) return;

    const slot = this[payload.operation];
    slot.percent = Math.max(0, Math.min(payload.percent, 100));
    slot.processedBytes = payload.processedBytes;
    slot.totalBytes = payload.totalBytes;
    slot.throughputMiBs = payload.throughputMiBs;
    slot.etaSeconds = payload.etaSeconds;
    slot.done = payload.done;
    // Backend errors arrive in Chinese; map them to the active locale for display.
    slot.error = payload.error ? translateBackendText(payload.error) : payload.error;
    slot.visible = true;
    slot.started = true;
  }

  reset(kind: ProgressKind, label = ''): void {
    if (!isTracked(kind)) return;
    this[kind] = { ...emptyProgress(), visible: true, label };
  }

  /**
   * Mark the task as failed.
   *
   * Frontend-side failures (validation, IPC rejections) never trigger the
   * backend's `done` event; without this step the bar would sit forever at
   * the last received percentage.
   */
  fail(kind: ProgressKind, message: string): void {
    if (!isTracked(kind)) return;
    const slot = this[kind];
    slot.done = true;
    slot.error = message;
    slot.visible = true;
  }

  /** Successful completion: fill the bar; a failed bar is left untouched. */
  succeed(kind: ProgressKind): void {
    if (!isTracked(kind)) return;
    const slot = this[kind];
    if (slot.error) return;
    slot.done = true;
    slot.percent = 100;
  }

  hide(kind: ProgressKind): void {
    if (!isTracked(kind)) return;
    this[kind].visible = false;
  }
}

export const progress = new ProgressStore();

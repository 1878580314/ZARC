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
  /** 至少收到一个后端事件后为 true；此前进度条以「准备中」的不确定状态运行。 / True once at least one backend event has arrived; before that the bar runs in the indeterminate "preparing" state. */
  started: boolean;
  /** 任务标签，让任务中心区分「压缩中」与「读取归档列表」。 / Task label, letting the Task Hub tell "compressing" apart from "reading the archive listing". */
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

/** 后端仅为压缩/解压发出进度事件；基准测试由任务 store 驱动。 / The backend only emits progress events for compress/decompress; benchmark is driven by the task store. */
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
    // 后端错误以中文返回；映射到活动语言后再显示。
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
   * 将任务标记为失败。
   *
   * 前端侧失败（校验、IPC 拒绝）不会触发后端的 `done` 事件；
   * 若没有这一步，进度条会永远停在最后收到的百分比。
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

  /** 成功完成：填满进度条；失败的进度条则保持原样。 / Successful completion: fill the bar; a failed bar is left untouched. */
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

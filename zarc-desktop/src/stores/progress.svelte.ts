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
  /** 已收到至少一次后端事件；未收到前进度条走「准备中」的不确定态。 */
  started: boolean;
  /** 任务标题，用于任务中心区分「正在压缩」与「正在读取归档列表」。 */
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

/** 后端只为 compress/decompress 发进度事件，benchmark 由 task store 驱动。 */
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
    slot.error = payload.error;
    slot.visible = true;
    slot.started = true;
  }

  reset(kind: ProgressKind, label = ''): void {
    if (!isTracked(kind)) return;
    this[kind] = { ...emptyProgress(), visible: true, label };
  }

  /**
   * 把任务标记为失败。
   *
   * 前端侧的失败（参数校验、IPC 拒绝）不会触发后端的 `done` 事件，
   * 没有这一步进度条会永远停在最后一次收到的百分比上。
   */
  fail(kind: ProgressKind, message: string): void {
    if (!isTracked(kind)) return;
    const slot = this[kind];
    slot.done = true;
    slot.error = message;
    slot.visible = true;
  }

  /** 任务成功收尾：补满进度条；已经处于失败态则保持不动。 */
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

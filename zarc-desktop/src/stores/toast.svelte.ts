export type ToastLevel = 'info' | 'success' | 'warn' | 'error';

export interface Toast {
  id: number;
  level: ToastLevel;
  title: string;
  detail?: string;
  /** 0 表示 toast 保留至用户手动关闭。 / 0 means the toast stays until the user closes it. */
  timeout: number;
}

const DEFAULT_TIMEOUT: Record<ToastLevel, number> = {
  info: 3200,
  success: 3600,
  warn: 6000,
  // 错误从不自动消失：用户常需复制完整错误链。 / Errors never auto-dismiss: users often need to copy out the full error chain.
  error: 0
};

const MAX_VISIBLE = 4;

class ToastStore {
  items = $state<Toast[]>([]);

  #seq = 0;
  #timers = new Map<number, ReturnType<typeof setTimeout>>();

  push(level: ToastLevel, title: string, detail?: string, timeout?: number): number {
    const id = ++this.#seq;
    const resolved = timeout ?? DEFAULT_TIMEOUT[level];
    this.items = [...this.items, { id, level, title, detail, timeout: resolved }];

    // 丢弃最旧的，防止长任务刷爆屏幕。 / Drop the oldest ones so a long task can't flood the screen.
    while (this.items.length > MAX_VISIBLE) {
      this.dismiss(this.items[0].id);
    }

    if (resolved > 0) {
      this.#timers.set(
        id,
        setTimeout(() => this.dismiss(id), resolved)
      );
    }
    return id;
  }

  info = (title: string, detail?: string) => this.push('info', title, detail);
  success = (title: string, detail?: string) => this.push('success', title, detail);
  warn = (title: string, detail?: string) => this.push('warn', title, detail);
  error = (title: string, detail?: string) => this.push('error', title, detail);

  dismiss(id: number): void {
    const timer = this.#timers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      this.#timers.delete(id);
    }
    this.items = this.items.filter((item) => item.id !== id);
  }

  clear(): void {
    for (const timer of this.#timers.values()) {
      clearTimeout(timer);
    }
    this.#timers.clear();
    this.items = [];
  }
}

export const toasts = new ToastStore();

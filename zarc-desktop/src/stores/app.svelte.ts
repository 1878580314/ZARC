import type { EmbeddedArchiveInfo, PathInfo, ViewId } from '../lib/api';
import { api } from '../lib/api';
import type { PathKind } from '../lib/format';

export type StatusLevel = 'idle' | 'busy' | 'success' | 'error';

export interface AppStatus {
  message: string;
  level: StatusLevel;
}

class AppStore {
  currentView = $state<ViewId>('compress');
  sfxInfo = $state<EmbeddedArchiveInfo | null>(null);
  status = $state<AppStatus>({ message: '就绪', level: 'idle' });

  // 跨视图共享的数据源；这里是唯一真相，视图不再各自持有副本。
  compressSource = $state('');
  compressKind = $state<PathKind>('文件');
  decompressSource = $state('');
  benchmarkSource = $state('');
  benchmarkKind = $state<PathKind>('文件');

  /** 压缩源的实测体积，用于在提交前展示预期规模。 */
  compressInfo = $state<PathInfo | null>(null);
  compressInfoLoading = $state(false);

  /** 压缩等级放在 store 里，性能测试页才能把推荐值一键写过来。 */
  compressLevel = $state(8);

  /** 快捷键面板的开关，由侧边栏按钮和 Ctrl+/ 共同控制。 */
  shortcutsOpen = $state(false);

  /** 递增序号，用于丢弃过期的 inspect 结果，避免快速换源时旧结果覆盖新结果。 */
  #inspectSeq = 0;

  get isSfx(): boolean {
    return this.sfxInfo !== null;
  }

  setView(view: ViewId): void {
    this.currentView = view;
  }

  setStatus(message: string, level: StatusLevel): void {
    this.status = { message, level };
  }

  setCompressSource(path: string, kind: PathKind): void {
    this.compressSource = path;
    this.compressKind = kind;
    void this.#measureCompressSource(path);
  }

  setDecompressSource(path: string): void {
    this.decompressSource = path;
  }

  setBenchmarkSource(path: string, kind: PathKind): void {
    this.benchmarkSource = path;
    this.benchmarkKind = kind;
  }

  /** 采纳性能测试给出的推荐等级并跳回压缩页。 */
  applyRecommendedLevel(level: number): void {
    this.compressLevel = level;
    this.setView('compress');
    this.setStatus(`已采用推荐等级 L${level}。`, 'success');
  }

  async #measureCompressSource(path: string): Promise<void> {
    const seq = ++this.#inspectSeq;
    if (!path) {
      this.compressInfo = null;
      this.compressInfoLoading = false;
      return;
    }
    this.compressInfoLoading = true;
    try {
      const info = await api.inspectPath(path);
      if (seq !== this.#inspectSeq) return;
      this.compressInfo = info;
      if (info.exists) {
        this.compressKind = info.isDir ? '目录' : '文件';
      }
    } catch {
      if (seq !== this.#inspectSeq) return;
      this.compressInfo = null;
    } finally {
      if (seq === this.#inspectSeq) {
        this.compressInfoLoading = false;
      }
    }
  }

  async initSfx(): Promise<void> {
    try {
      const info = await api.getEmbeddedInfo();
      if (!info) return;
      this.sfxInfo = info;
      this.decompressSource = info.hostPath;
      this.setView('decompress');
      this.setStatus('已进入自解压模式，请选择输出目录。', 'success');
    } catch (error) {
      console.error('Failed to detect embedded archive mode', error);
    }
  }
}

export const app = new AppStore();

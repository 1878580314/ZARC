import type { EmbeddedArchiveInfo, ViewId } from '../lib/api';
import { api } from '../lib/api';

export type StatusLevel = 'idle' | 'busy' | 'success' | 'error';

export interface AppStatus {
  message: string;
  level: StatusLevel;
}

class AppStore {
  currentView = $state<ViewId>('compress');
  sfxInfo = $state<EmbeddedArchiveInfo | null>(null);
  status = $state<AppStatus>({ message: '就绪', level: 'idle' });

  // Sources shared across views (kept here so drag-drop can populate them).
  compressSource = $state('');
  compressKind = $state<'文件' | '目录'>('文件');
  decompressSource = $state('');
  benchmarkSource = $state('');
  benchmarkKind = $state<'文件' | '目录'>('文件');

  get isSfx(): boolean {
    return this.sfxInfo !== null;
  }

  setView(view: ViewId): void {
    this.currentView = view;
  }

  setStatus(message: string, level: StatusLevel): void {
    this.status = { message, level };
  }

  setCompressSource(path: string, kind: '文件' | '目录'): void {
    this.compressSource = path;
    this.compressKind = kind;
  }

  setDecompressSource(path: string): void {
    this.decompressSource = path;
  }

  setBenchmarkSource(path: string, kind: '文件' | '目录'): void {
    this.benchmarkSource = path;
    this.benchmarkKind = kind;
  }

  async initSfx(): Promise<void> {
    try {
      const info = await api.getEmbeddedInfo();
      if (!info) return;
      this.sfxInfo = info;
      this.decompressSource = info.hostPath;
      this.setView('decompress');
      this.setStatus('已进入自解压模式。请选择输出目录后开始解压。', 'success');
    } catch (error) {
      console.error('Failed to detect embedded archive mode', error);
    }
  }
}

export const app = new AppStore();

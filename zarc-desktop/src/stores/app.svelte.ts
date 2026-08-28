import type { EmbeddedArchiveInfo, PathInfo, ViewId } from '../lib/api';
import { api } from '../lib/api';
import type { PathKind } from '../lib/format';
import { t } from '../lib/i18n/index.svelte';

export type StatusLevel = 'idle' | 'busy' | 'success' | 'error';

export interface AppStatus {
  message: string;
  level: StatusLevel;
}

class AppStore {
  currentView = $state<ViewId>('compress');
  sfxInfo = $state<EmbeddedArchiveInfo | null>(null);
  status = $state<AppStatus>({ message: t('status.ready'), level: 'idle' });

  // Data sources shared across views; this is the single source of truth, and
  // views no longer keep their own copies.
  compressSource = $state('');
  compressKind = $state<PathKind>('file');
  decompressSource = $state('');
  benchmarkSource = $state('');
  benchmarkKind = $state<PathKind>('file');

  /** Measured size of the compression source, shown before submitting to set expectations. */
  compressInfo = $state<PathInfo | null>(null);
  compressInfoLoading = $state(false);

  /** The compression level lives in the store so the benchmark view can write its recommendation here in one click. */
  compressLevel = $state(8);

  /** Shortcuts panel toggle, driven by the sidebar button and Ctrl+/. */
  shortcutsOpen = $state(false);

  /** Incrementing sequence number for discarding stale inspect results, so older ones never overwrite newer ones while the source changes rapidly. */
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

  /** Apply the level recommended by the benchmark and jump back to the Compress view. */
  applyRecommendedLevel(level: number): void {
    this.compressLevel = level;
    this.setView('compress');
    this.setStatus(t('status.recommendApplied', { level }), 'success');
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
        this.compressKind = info.isDir ? 'folder' : 'file';
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
      this.setStatus(t('status.sfxMode'), 'success');
    } catch (error) {
      console.error('Failed to detect embedded archive mode', error);
    }
  }
}

export const app = new AppStore();

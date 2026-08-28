import { getCurrentWebview } from '@tauri-apps/api/webview';
import { app } from '../stores/app.svelte';
import { toasts } from '../stores/toast.svelte';
import { isArchivePath, pathBaseName, pathKindLabel } from './format';
import { t } from './i18n/index.svelte';

class DragDropState {
  visible = $state(false);
  /** Number of dragged-in files, used to show a precise hint on the overlay. */
  count = $state(0);
}

export const dropOverlay = new DragDropState();

async function route(path: string): Promise<void> {
  const kind = await pathKindLabel(path);
  const name = pathBaseName(path);
  const kindLabel = t(kind === 'folder' ? 'kind.folder' : 'kind.file');

  // Fill both data sources whichever branch runs, so users don't have to
  // re-select when switching tabs.
  app.setCompressSource(path, kind);
  app.setBenchmarkSource(path, kind);

  if (kind === 'file' && isArchivePath(path)) {
    app.setDecompressSource(path);
    app.setView('decompress');
    toasts.info(t('compress.drop.archiveLoaded', { name }), t('compress.drop.archiveLoadedHint'));
  } else {
    app.setView('compress');
    toasts.info(t('compress.drop.kindLoaded', { kind: kindLabel, name }));
  }
}

export async function initDragDrop(): Promise<() => void> {
  return getCurrentWebview().onDragDropEvent((event) => {
    if (app.isSfx) return;

    const payload = event.payload;
    if (payload.type === 'enter' || payload.type === 'over') {
      dropOverlay.visible = true;
      // The `over` event carries no paths; keep the count recorded on `enter`.
      if ('paths' in payload) {
        dropOverlay.count = payload.paths.length;
      }
    } else if (payload.type === 'leave') {
      dropOverlay.visible = false;
      dropOverlay.count = 0;
    } else if (payload.type === 'drop') {
      dropOverlay.visible = false;
      dropOverlay.count = 0;
      if (payload.paths.length === 0) return;
      if (payload.paths.length > 1) {
        toasts.warn(t('toast.onePathOnly'), t('compress.drop.multiIgnored', { count: payload.paths.length - 1 }));
      }
      void route(payload.paths[0]);
    }
  });
}

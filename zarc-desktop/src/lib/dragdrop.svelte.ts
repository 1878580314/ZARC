import { getCurrentWebview } from '@tauri-apps/api/webview';
import { app } from '../stores/app.svelte';
import { toasts } from '../stores/toast.svelte';
import { isArchivePath, pathBaseName, type PathKind } from './format';
import { t } from './i18n/index.svelte';

class DragDropState {
  visible = $state(false);
  /** 拖入的文件数，用于在覆盖层上显示精确提示。 / Number of dragged-in files, used to show a precise hint on the overlay. */
  count = $state(0);
}

export const dropOverlay = new DragDropState();

/** 无 IPC 时的种类启发式：`release.v2/` 这类带点目录会被误判为文件，
 *  只有当测量失败/路径不存在时才使用。 / Kind heuristic without IPC... */
function guessKind(path: string): PathKind {
  return pathBaseName(path).includes('.') ? 'file' : 'folder';
}

async function route(path: string): Promise<void> {
  // setCompressSource 内已做过一次 inspect，直接复用其结果做路由，不再单独调一次 IPC。
  // setCompressSource already inspects once; reuse its result for routing instead of a second IPC.
  const info = await app.setCompressSource(path, guessKind(path));
  const kind: PathKind = info && info.exists ? (info.isDir ? 'folder' : 'file') : guessKind(path);
  const name = pathBaseName(path);
  const kindLabel = t(kind === 'folder' ? 'kind.folder' : 'kind.file');

  // 无论走哪个分支都填充基准数据源，这样切换标签页时无需重新选择。
  // Fill the benchmark source whichever branch runs, so users don't have to
  // re-select when switching tabs. (Compress source was already set + measured above.)
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
      // `over` 事件不含路径；沿用 `enter` 时记录的数量。
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

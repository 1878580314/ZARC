import { getCurrentWebview } from '@tauri-apps/api/webview';
import { app } from '../stores/app.svelte';
import { toasts } from '../stores/toast.svelte';
import { isArchivePath, pathBaseName, pathKindLabel } from './format';

class DragDropState {
  visible = $state(false);
  /** 拖入的文件数量，用于在遮罩上给出精确提示。 */
  count = $state(0);
}

export const dropOverlay = new DragDropState();

async function route(path: string): Promise<void> {
  const kind = await pathKindLabel(path);
  const name = pathBaseName(path);

  // 无论走哪条分支都把两侧数据源填好，用户切换标签页时不必重新选择。
  app.setCompressSource(path, kind);
  app.setBenchmarkSource(path, kind);

  if (kind === '文件' && isArchivePath(path)) {
    app.setDecompressSource(path);
    app.setView('decompress');
    toasts.info(`已载入归档 ${name}`, '如需压缩它，切换到「压缩」页即可。');
  } else {
    app.setView('compress');
    toasts.info(`已载入${kind} ${name}`);
  }
}

export async function initDragDrop(): Promise<() => void> {
  return getCurrentWebview().onDragDropEvent((event) => {
    if (app.isSfx) return;

    const payload = event.payload;
    if (payload.type === 'enter' || payload.type === 'over') {
      dropOverlay.visible = true;
      // `over` 事件不带 paths，沿用 `enter` 时记录的数量。
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
        toasts.warn('一次只能处理一个路径', `已选用第一项，其余 ${payload.paths.length - 1} 项被忽略。`);
      }
      void route(payload.paths[0]);
    }
  });
}

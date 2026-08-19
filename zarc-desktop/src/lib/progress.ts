import { listen } from '@tauri-apps/api/event';
import type { ProgressPayload } from './api';
import { progress } from '../stores/progress.svelte';

export async function initProgressListener(): Promise<() => void> {
  const unlisten = await listen<ProgressPayload>('zarc://progress', (event) => {
    progress.update(event.payload);
  });
  return unlisten;
}

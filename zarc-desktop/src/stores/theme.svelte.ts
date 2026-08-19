export type ThemeMode = 'dark' | 'light';

const STORAGE_KEY = 'theme';

class ThemeStore {
  current = $state<ThemeMode>('dark');
  /** 用户从未手动选过时跟随系统；一旦手动切换就固定下来。 */
  followSystem = $state(true);

  #media: MediaQueryList | null = null;
  #onSystemChange = (event: MediaQueryListEvent): void => {
    if (!this.followSystem) return;
    this.current = event.matches ? 'light' : 'dark';
  };

  init(): () => void {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'dark' || stored === 'light') {
      this.current = stored;
      this.followSystem = false;
    } else if (typeof matchMedia !== 'undefined') {
      this.current = matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    }

    if (typeof matchMedia !== 'undefined') {
      this.#media = matchMedia('(prefers-color-scheme: light)');
      this.#media.addEventListener('change', this.#onSystemChange);
    }

    return () => this.#media?.removeEventListener('change', this.#onSystemChange);
  }

  toggle(): void {
    this.set(this.current === 'dark' ? 'light' : 'dark');
  }

  set(mode: ThemeMode): void {
    this.current = mode;
    this.followSystem = false;
    localStorage.setItem(STORAGE_KEY, mode);
  }
}

export const theme = new ThemeStore();

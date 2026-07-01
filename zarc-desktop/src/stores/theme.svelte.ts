class ThemeStore {
  current = $state<'dark' | 'light'>('dark');

  init(): void {
    const stored = localStorage.getItem('theme');
    if (stored === 'dark' || stored === 'light') {
      this.current = stored;
    } else if (typeof matchMedia !== 'undefined') {
      this.current = matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    }
  }

  toggle(): void {
    this.current = this.current === 'dark' ? 'light' : 'dark';
    localStorage.setItem('theme', this.current);
  }
}

export const theme = new ThemeStore();

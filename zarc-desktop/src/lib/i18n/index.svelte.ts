import { DICT } from './dicts';

export type Locale = 'zh' | 'en';

const STORAGE_KEY = 'zarc.locale';

export const LOCALES: { id: Locale; label: string }[] = [
  { id: 'zh', label: '中文' },
  { id: 'en', label: 'English' }
];

function initialLocale(): Locale {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === 'zh' || saved === 'en') return saved;
  } catch {
    // 无存储（或被禁用）；回落到默认值。
    // No storage (or blocked); fall through to the default.
  }
  return 'zh';
}

export const i18n = $state({ locale: initialLocale() as Locale });

/** 解析当前语言，并在其变化时让调用方重新运行。 / Resolve the current locale and re-run the caller when it changes. */
export function currentLocale(): Locale {
  return i18n.locale;
}

/** 跟随界面语言的数字格式化 BCP-47 标签。 / BCP-47 tag for number formatting that follows the UI language. */
export function numberLocale(): string {
  return i18n.locale === 'zh' ? 'zh-CN' : 'en-US';
}

export function setLocale(locale: Locale): void {
  i18n.locale = locale;
  try {
    localStorage.setItem(STORAGE_KEY, locale);
  } catch {
    // 持久化尽力而为；内存中的语言仍然生效。
    // Persistence is best-effort; the in-memory locale still applies.
  }
}

export function toggleLocale(): void {
  setLocale(i18n.locale === 'zh' ? 'en' : 'zh');
}

/**
 * 在活动语言中查找词典键，支持插值。
 *
 * `{name}` 占位符由 `params` 替换。缺失的键先回落到中文，
 * 再回落到键本身，因此遗漏的翻译绝不会让界面空白。
 * Look up a dictionary key in the active locale, with interpolation.
 *
 * `{name}` placeholders are replaced from `params`. Missing keys fall back to
 * Chinese, then to the key itself, so a forgotten translation can never blank
 * out the UI.
 */
export function t(key: string, params?: Record<string, string | number>): string {
  const table = DICT[i18n.locale];
  const raw = table[key] ?? DICT.zh[key] ?? DICT.en[key] ?? key;
  if (!params) return raw;
  return raw.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match
  );
}

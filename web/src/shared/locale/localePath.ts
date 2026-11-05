import { useI18n } from 'vue-i18n';

// English pages live at the root (/guides), the others under their language code (/vi/guides), so search engines index
// every language.

export const LOCALES = ['en', 'vi', 'zh', 'hi'] as const;
export type AppLocale = (typeof LOCALES)[number];

function hasPrefix(path: string, locale: AppLocale): boolean {
  return path === `/${locale}` || path.startsWith(`/${locale}/`);
}

/** The language a page address is in. */
export function pathLocale(path: string): AppLocale {
  return LOCALES.find((locale) => locale !== 'en' && hasPrefix(path, locale)) ?? 'en';
}

/** The page's address without its language prefix: /vi/guides → /guides, /vi → /. */
export function basePath(path: string): string {
  const locale = pathLocale(path);
  return locale === 'en' ? path : path.slice(locale.length + 1) || '/';
}

/** The address of `path` in `locale`: /guides in vi → /vi/guides, / in vi → /vi, anything in en → itself. */
export function localizedPath(path: string, locale: string): string {
  const base = basePath(path);
  if (locale === 'en' || !LOCALES.some((known) => known === locale)) {
    return base;
  }
  return base === '/' ? `/${locale}` : `/${locale}${base}`;
}

/** Turns a page path into its address in the language on screen; used for every link between pages. */
export function useLocalePath() {
  const { locale } = useI18n();
  return (path: string) => localizedPath(path, locale.value);
}

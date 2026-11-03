import { useI18n } from 'vue-i18n';

// English pages live at the root (/guides), Vietnamese ones under /vi (/vi/guides), so search engines index both.

/** Prefix of the Vietnamese pages. */
export const VI_PREFIX = '/vi';

/** True for /vi and anything under /vi/. */
export function isViPath(path: string): boolean {
  return path === VI_PREFIX || path.startsWith(`${VI_PREFIX}/`);
}

/** The page's address without its language prefix: /vi/guides → /guides, /vi → /. */
export function basePath(path: string): string {
  return isViPath(path) ? path.slice(VI_PREFIX.length) || '/' : path;
}

/** The address of `path` in `locale`: /guides in vi → /vi/guides, / in vi → /vi, anything in en → itself. */
export function localizedPath(path: string, locale: string): string {
  const base = basePath(path);
  if (locale !== 'vi') {
    return base;
  }
  return base === '/' ? VI_PREFIX : `${VI_PREFIX}${base}`;
}

/** The language a page address is in. */
export function pathLocale(path: string): 'en' | 'vi' {
  return isViPath(path) ? 'vi' : 'en';
}

/** Turns a page path into its address in the language on screen; used for every link between pages. */
export function useLocalePath() {
  const { locale } = useI18n();
  return (path: string) => localizedPath(path, locale.value);
}

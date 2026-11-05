import { watch } from 'vue';
import { createI18n } from 'vue-i18n';

import aboutEn from '@/features/about/locales/en.json';
import aboutVi from '@/features/about/locales/vi.json';
import checkerEn from '@/features/checker/locales/en.json';
import checkerVi from '@/features/checker/locales/vi.json';
import convertEn from '@/features/convert/locales/en.json';
import convertVi from '@/features/convert/locales/vi.json';
import guidesEn from '@/features/guides/locales/en.json';
import guidesVi from '@/features/guides/locales/vi.json';
import legalEn from '@/features/legal/locales/en.json';
import legalVi from '@/features/legal/locales/vi.json';
import { type AppLocale, LOCALES } from '@/shared/locale/localePath';
import en from '@/shared/locales/en.json';
import vi from '@/shared/locales/vi.json';

export { LOCALES };
export type { AppLocale };

/** Languages downloaded only when they are shown, so nobody else pays for their text. */
const LAZY_MESSAGES: Record<string, (() => Promise<{ default: object }>) | undefined> = {
  zh: () => import('./messages/zh'),
  hi: () => import('./messages/hi'),
};

/** What `ensureLocale` needs of an i18n instance. */
interface MessageTarget {
  availableLocales: readonly string[];
  setLocaleMessage(locale: string, message: never): void;
}

/** Makes sure the text of `locale` is there before it is shown. If it cannot be fetched, the page shows English. */
export async function ensureLocale(target: MessageTarget, locale: string): Promise<void> {
  const load = LAZY_MESSAGES[locale];
  if (!load || target.availableLocales.includes(locale)) {
    return;
  }
  try {
    target.setLocaleMessage(locale, (await load()).default as never);
  } catch {
    // The fallback language covers every key.
  }
}

export const LOCALE_STORAGE_KEY = 'revit-downgrade.locale';

/** The two storage methods the app uses; `localStorage` satisfies it. */
export interface LocaleStorage {
  getItem: (key: string) => string | null;
  setItem: (key: string, value: string) => void;
}

function isLocale(value: string | null): value is AppLocale {
  return LOCALES.some((locale) => locale === value);
}

/** A remembered choice wins; otherwise the browser's language if the site has it, and English for everything else. */
export function pickLocale(browserLanguage: string, remembered: string | null): AppLocale {
  if (isLocale(remembered)) {
    return remembered;
  }
  const language = browserLanguage.toLowerCase().split('-')[0] ?? '';
  return isLocale(language) ? language : 'en';
}

function readRemembered(storage: LocaleStorage): string | null {
  try {
    return storage.getItem(LOCALE_STORAGE_KEY);
  } catch {
    return null;
  }
}

const english = { ...en, ...convertEn, ...aboutEn, ...guidesEn, ...checkerEn, ...legalEn };

/** The languages every visitor downloads, under a string key: the others join them through `ensureLocale`. */
const messages: Record<string, typeof english> = {
  en: english,
  vi: { ...vi, ...convertVi, ...aboutVi, ...guidesVi, ...checkerVi, ...legalVi },
};

export function createAppI18n(browserLanguage: string, storage: LocaleStorage) {
  const i18n = createI18n({
    legacy: false,
    locale: pickLocale(browserLanguage, readRemembered(storage)),
    fallbackLocale: 'en',
    messages,
  });
  const reflectLocale = (locale: string) => {
    // There is no document while prerendering; the page template already says lang="en".
    if (typeof document !== 'undefined') {
      document.documentElement.lang = locale;
    }
  };
  reflectLocale(i18n.global.locale.value);
  watch(
    i18n.global.locale,
    (locale) => {
      reflectLocale(locale);
      try {
        storage.setItem(LOCALE_STORAGE_KEY, locale);
      } catch {
        // Storage can be blocked (private window, site data off); the choice then lasts for this visit only.
      }
    },
    { flush: 'sync' },
  );
  return i18n;
}

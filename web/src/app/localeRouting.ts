import type { Router } from 'vue-router';

import { localizedPath, pathLocale } from '@/shared/locale/localePath';

import { type createAppI18n, ensureLocale } from './i18n';

/**
 * Keeps the language on screen in step with the address: /vi/... is Vietnamese, a page without a prefix English. A
 * job's address (/job/<id>) keeps whatever language was chosen. With `preferred` set, the first page opened without a
 * language prefix moves to the visitor's language (their earlier choice, or their browser's) — the prerender step
 * leaves it unset, so every English page is rendered in English.
 */
export function installLocaleRouting(router: Router, i18n: ReturnType<typeof createAppI18n>, preferred?: string) {
  let first = true;
  router.beforeEach(async (to) => {
    const wasFirst = first;
    first = false;
    if (to.name === 'job') {
      await ensureLocale(i18n.global, i18n.global.locale.value);
      return true;
    }
    const locale = pathLocale(to.path);
    if (wasFirst && preferred && preferred !== 'en' && locale === 'en' && to.name !== 'notFound') {
      return { path: localizedPath(to.path, preferred), query: to.query, hash: to.hash, replace: true };
    }
    await ensureLocale(i18n.global, locale);
    i18n.global.locale.value = locale;
    return true;
  });
}

import type { Router } from 'vue-router';

import { localizedPath, pathLocale } from '@/shared/locale/localePath';

import type { createAppI18n } from './i18n';

/**
 * Keeps the language on screen in step with the address: /vi/... is Vietnamese, every other page English. A job's
 * address (/job/<id>) keeps whatever language was chosen. With `preferred` set, the first page opened without a
 * language prefix moves to the visitor's language (their earlier choice, or a Vietnamese browser) — the prerender
 * step leaves it unset, so every English page is rendered in English.
 */
export function installLocaleRouting(router: Router, i18n: ReturnType<typeof createAppI18n>, preferred?: string) {
  let first = true;
  router.beforeEach((to) => {
    const wasFirst = first;
    first = false;
    if (to.name === 'job') {
      return true;
    }
    const locale = pathLocale(to.path);
    if (wasFirst && preferred === 'vi' && locale === 'en' && to.name !== 'notFound') {
      return { path: localizedPath(to.path, 'vi'), query: to.query, hash: to.hash, replace: true };
    }
    i18n.global.locale.value = locale;
    return true;
  });
}

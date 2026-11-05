import { createRouter, type RouteLocationNormalized, type RouterHistory } from 'vue-router';

import { guideBySlug } from '@/shared/catalog/guides';
import { isPair } from '@/shared/catalog/pairs';

// Each view is its own chunk, so a page only downloads the code it shows.
const ConvertView = () => import('@/features/convert/ConvertView.vue');
const AboutView = () => import('@/features/about/AboutView.vue');
const GuidesView = () => import('@/features/guides/GuidesView.vue');
const GuideView = () => import('@/features/guides/GuideView.vue');
const CheckerView = () => import('@/features/checker/CheckerView.vue');
const LegalView = () => import('@/features/legal/LegalView.vue');
const NotFoundView = () => import('@/features/not-found/NotFoundView.vue');

/** Shows the "page not found" view at the address that was asked for. */
function notFound(to: RouteLocationNormalized) {
  return { name: 'notFound', params: { unknown: to.path.slice(1).split('/') }, query: to.query, hash: to.hash };
}

/** Every page exists in English at the root and in each other language under its code (see @/shared/locale/localePath). */
const LANG = '/:lang(vi|zh|hi)?';

export function createAppRouter(history: RouterHistory) {
  return createRouter({
    history,
    routes: [
      { path: LANG, name: 'convert', component: ConvertView },
      { path: `${LANG}/how-it-works`, name: 'about', component: AboutView },
      // One page per downgrade (/revit-2026-to-2024), so each can be found by searching for it.
      {
        path: `${LANG}/revit-:from(\\d{4})-to-:to(\\d{4})`,
        name: 'pair',
        component: ConvertView,
        beforeEnter: (to) => isPair(Number(to.params.from), Number(to.params.to)) || notFound(to),
      },
      { path: `${LANG}/guides`, name: 'guides', component: GuidesView },
      {
        path: `${LANG}/guides/:slug`,
        name: 'guide',
        component: GuideView,
        beforeEnter: (to) => (guideBySlug(String(to.params.slug)) ? true : notFound(to)),
      },
      { path: `${LANG}/revit-version-checker`, name: 'checker', component: CheckerView },
      { path: `${LANG}/privacy`, name: 'privacy', component: LegalView, props: { page: 'privacy' } },
      { path: `${LANG}/terms`, name: 'terms', component: LegalView, props: { page: 'terms' } },
      // A conversion's address belongs to one visitor and stays in the language they chose.
      { path: '/job/:id', name: 'job', component: ConvertView },
      { path: '/:unknown(.*)*', name: 'notFound', component: NotFoundView },
    ],
    scrollBehavior: (to, _from, saved) => saved ?? (to.hash ? { el: to.hash } : { top: 0 }),
  });
}

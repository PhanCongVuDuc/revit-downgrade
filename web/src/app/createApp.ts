import { createPinia } from 'pinia';
import { createApp } from 'vue';
import { createMemoryHistory, createWebHistory } from 'vue-router';

import App from '@/App.vue';
import { safeStorage } from '@/shared/storage/safeStorage';

import { createAppI18n } from './i18n';
import { installLocaleRouting } from './localeRouting';
import { createAppRouter } from './router';

/** The app in the browser: the address bar drives the route and the language of the page. */
export function createRevitDowngradeApp() {
  const app = createApp(App);
  const router = createAppRouter(createWebHistory());
  const i18n = createAppI18n(navigator.language, safeStorage());
  // The language picked from the visitor's earlier choice or browser, before the address has a say.
  installLocaleRouting(router, i18n, i18n.global.locale.value);
  app.use(createPinia());
  app.use(router);
  app.use(i18n);
  return { app, router };
}

/** The app as the prerender step renders it: no remembered state, routes from memory, language from the address. */
export function createPrerenderApp(createSsrApp: typeof createApp) {
  const app = createSsrApp(App);
  const router = createAppRouter(createMemoryHistory());
  const i18n = createAppI18n('en', { getItem: () => null, setItem: () => undefined });
  installLocaleRouting(router, i18n);
  app.use(createPinia());
  app.use(router);
  app.use(i18n);
  return { app, router };
}

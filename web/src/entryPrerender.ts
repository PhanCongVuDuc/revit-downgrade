// Entry of the prerender build (see build/prerender.ts): renders one page of the app to HTML, in English, at build
// time, so every page is served with its own content and head tags before any JavaScript runs.
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';

import { createPrerenderApp } from '@/app/createApp';
import type { PageMeta, PrerenderContext } from '@/shared/seo/pageMeta';

export { NOT_FOUND_PATH, SITE_PAGES } from '@/app/pages';

export interface RenderedPage {
  html: string;
  meta: PageMeta | undefined;
}

export async function render(path: string): Promise<RenderedPage> {
  const { app, router } = createPrerenderApp(createSSRApp);
  await router.push(path);
  await router.isReady();
  const context: PrerenderContext = {};
  const html = await renderToString(app, context);
  return { html, meta: context.meta };
}

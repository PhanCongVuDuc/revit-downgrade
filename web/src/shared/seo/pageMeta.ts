import { useSSRContext, watchEffect } from 'vue';

import { localizedPath, pathLocale } from '@/shared/locale/localePath';

/** Address the site is published at; canonical and share links point here. */
export const SITE_URL = 'https://revitdowngrade.com';

export const SITE_NAME = 'Revit Downgrade';

/** What a page tells search engines and link previews about itself. */
export interface PageMeta {
  title: string;
  description: string;
  /** Path of the page's own address, e.g. /revit-2026-to-2024. */
  path: string;
  /** Schema.org JSON-LD objects describing the page. */
  jsonLd?: object[];
  /** Pages that should not appear in search results (a job's address, a missing page). */
  noindex?: boolean;
  /** An article's dates (ISO yyyy-mm-dd): the page is shared as an article rather than a website. */
  article?: { published: string; modified: string };
}

/** Description of the shared link-preview picture, in each language. */
export const IMAGE_ALT: Record<'en' | 'vi', string> = {
  en: 'Revit Downgrade: open a newer Revit file in an older Revit',
  vi: 'Revit Downgrade: mở file Revit mới hơn bằng Revit cũ hơn',
};

/** Handed to the app by the prerender step, which reads the meta of the page it rendered from it. */
export interface PrerenderContext {
  meta?: PageMeta;
}

export function pageUrl(path: string): string {
  return `${SITE_URL}${path}`;
}

/** The page in each language, for `hreflang` links; English is also the default. */
export function pageAlternates(path: string): { hreflang: string; href: string }[] {
  const en = pageUrl(localizedPath(path, 'en'));
  return [
    { hreflang: 'en', href: en },
    { hreflang: 'vi', href: pageUrl(localizedPath(path, 'vi')) },
    { hreflang: 'x-default', href: en },
  ];
}

function setAlternates(path: string | null) {
  for (const old of document.head.querySelectorAll('link[rel="alternate"][hreflang]')) {
    old.remove();
  }
  for (const { hreflang, href } of path === null ? [] : pageAlternates(path)) {
    const link = document.createElement('link');
    link.setAttribute('rel', 'alternate');
    link.setAttribute('hreflang', hreflang);
    link.setAttribute('href', href);
    document.head.append(link);
  }
}

function setMeta(attribute: 'name' | 'property', key: string, value: string) {
  let tag = document.head.querySelector(`meta[${attribute}="${key}"]`);
  if (!tag) {
    tag = document.createElement('meta');
    tag.setAttribute(attribute, key);
    document.head.append(tag);
  }
  tag.setAttribute('content', value);
}

function setCanonical(url: string) {
  let link = document.head.querySelector('link[rel="canonical"]');
  if (!link) {
    link = document.createElement('link');
    link.setAttribute('rel', 'canonical');
    document.head.append(link);
  }
  link.setAttribute('href', url);
}

function setArticle(article: PageMeta['article']) {
  setMeta('property', 'og:type', article ? 'article' : 'website');
  for (const key of ['article:published_time', 'article:modified_time']) {
    document.head.querySelector(`meta[property="${key}"]`)?.remove();
  }
  if (article) {
    setMeta('property', 'article:published_time', article.published);
    setMeta('property', 'article:modified_time', article.modified);
  }
}

function setJsonLd(items: object[] | undefined) {
  for (const old of document.head.querySelectorAll('script[type="application/ld+json"]')) {
    old.remove();
  }
  for (const item of items ?? []) {
    const script = document.createElement('script');
    script.type = 'application/ld+json';
    script.textContent = JSON.stringify(item);
    document.head.append(script);
  }
}

/** Writes the page's meta into the document head, the same tags the prerendered page carries. */
export function applyPageMeta(meta: PageMeta) {
  document.title = meta.title;
  setMeta('name', 'description', meta.description);
  setMeta('property', 'og:title', meta.title);
  setMeta('property', 'og:description', meta.description);
  const vi = pathLocale(meta.path) === 'vi';
  setMeta('property', 'og:locale', vi ? 'vi_VN' : 'en_US');
  setMeta('property', 'og:image:alt', IMAGE_ALT[vi ? 'vi' : 'en']);
  setArticle(meta.article);
  // A page kept out of search has no address of its own to point to.
  if (meta.noindex) {
    setMeta('name', 'robots', 'noindex');
    document.head.querySelector('link[rel="canonical"]')?.remove();
    document.head.querySelector('meta[property="og:url"]')?.remove();
    setAlternates(null);
  } else {
    const url = pageUrl(meta.path);
    document.head.querySelector('meta[name="robots"]')?.remove();
    setMeta('property', 'og:url', url);
    setCanonical(url);
    setAlternates(meta.path);
  }
  setJsonLd(meta.jsonLd);
}

/** What the document head currently tells search engines: canonical address, robots rule and JSON-LD blocks. */
export function currentHeadMeta() {
  return {
    canonical: document.head.querySelector('link[rel="canonical"]')?.getAttribute('href') ?? null,
    robots: document.head.querySelector('meta[name="robots"]')?.getAttribute('content') ?? null,
    jsonLd: [...document.head.querySelectorAll('script[type="application/ld+json"]')].map((s) => s.textContent),
  };
}

/**
 * The page's title, description, address and structured data. While prerendering they are handed to the prerender
 * step, which writes them into the HTML; in the browser they follow the language and the route.
 */
export function usePageMeta(get: () => PageMeta) {
  if (import.meta.env.SSR) {
    const context = useSSRContext<PrerenderContext>();
    if (context) {
      context.meta = get();
    }
    return;
  }
  watchEffect(() => {
    applyPageMeta(get());
  });
}

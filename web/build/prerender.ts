// Prerenders every page of the site at build time. After the browser build, it builds src/entryPrerender.ts for Node,
// renders each page to HTML and writes it, with that page's title, description, canonical address and structured data
// in the head, to dist/pages/<path>.html. The server returns these files; the app then takes over in the browser.
// It also writes the 404 page and sitemap.xml from the same list of pages, so they cannot drift apart.
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import { build, type Plugin, type ResolvedConfig } from 'vite';

// The shapes src/entryPrerender.ts exports (PageMeta is in src/shared/seo/pageMeta.ts). They are restated here because
// this file is type-checked with the Node config, which does not resolve the app's `@/` imports.
interface PageMeta {
  title: string;
  description: string;
  path: string;
  jsonLd?: object[];
  noindex?: boolean;
}

interface RenderedPage {
  html: string;
  meta: PageMeta | undefined;
}

interface PrerenderEntry {
  render: (path: string) => Promise<RenderedPage>;
  SITE_PAGES: { path: string; updated: string }[];
  NOT_FOUND_PATH: string;
}

const SITE_URL = 'https://revitdowngrade.com';

function escapeHtml(text: string): string {
  return text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}

/** JSON inside a <script> element: `<` is escaped so no string in it can close the element. */
function scriptJson(value: object): string {
  return JSON.stringify(value).replaceAll('<', String.raw`<`);
}

/** Replaces exactly one match of `pattern` in `html`, so a changed template fails the build instead of a silent page. */
function replaceOnce(html: string, pattern: RegExp, replacement: string): string {
  const matches = html.match(new RegExp(pattern.source, 'g'));
  if (matches?.length !== 1) {
    throw new Error(
      `prerender: expected one match of ${String(pattern)} in index.html, found ${String(matches?.length ?? 0)}`,
    );
  }
  return html.replace(pattern, () => replacement);
}

/** The page's address in English and in Vietnamese (/vi prefix), as src/shared/locale/localePath.ts builds them. */
function alternates(path: string): { hreflang: string; href: string }[] {
  const vi = path === '/vi' || path.startsWith('/vi/');
  const base = vi ? path.slice(3) || '/' : path;
  const en = `${SITE_URL}${base}`;
  return [
    { hreflang: 'en', href: en },
    { hreflang: 'vi', href: `${SITE_URL}${base === '/' ? '/vi' : `/vi${base}`}` },
    { hreflang: 'x-default', href: en },
  ];
}

export function pageHtml(template: string, page: RenderedPage): string {
  const meta: PageMeta | undefined = page.meta;
  if (!meta) {
    throw new Error('prerender: a page rendered without calling usePageMeta');
  }
  const title = escapeHtml(meta.title);
  const description = escapeHtml(meta.description);
  const url = escapeHtml(`${SITE_URL}${meta.path}`);
  const vi = meta.path === '/vi' || meta.path.startsWith('/vi/');
  let html = template;
  if (vi) {
    html = replaceOnce(html, /<html lang="en">/, '<html lang="vi">');
    html = replaceOnce(
      html,
      /<meta property="og:locale" content="en_US" \/>/,
      '<meta property="og:locale" content="vi_VN" />',
    );
  }
  html = replaceOnce(html, /<title>[^<]*<\/title>/, `<title>${title}</title>`);
  html = replaceOnce(
    html,
    /<meta\s+name="description"\s+content="[^"]*"\s*\/>/,
    `<meta name="description" content="${description}" />`,
  );
  html = replaceOnce(
    html,
    /<meta\s+property="og:title"\s+content="[^"]*"\s*\/>/,
    `<meta property="og:title" content="${title}" />`,
  );
  html = replaceOnce(
    html,
    /<meta\s+property="og:description"\s+content="[^"]*"\s*\/>/,
    `<meta property="og:description" content="${description}" />`,
  );
  const head = [
    meta.noindex ? '<meta name="robots" content="noindex" />' : `<link rel="canonical" href="${url}" />`,
    meta.noindex ? '' : `<meta property="og:url" content="${url}" />`,
    ...(meta.noindex ? [] : alternates(meta.path)).map(
      (a) => `<link rel="alternate" hreflang="${a.hreflang}" href="${escapeHtml(a.href)}" />`,
    ),
    ...(meta.jsonLd ?? []).map((item) => `<script type="application/ld+json">${scriptJson(item)}</script>`),
  ]
    .filter(Boolean)
    .map((line) => `    ${line}\n`)
    .join('');
  html = replaceOnce(html, /\n[ \t]*<\/head>/, `\n${head}  </head>`);
  html = replaceOnce(html, /<div id="app"><\/div>/, `<div id="app">${page.html}</div>`);
  return html;
}

export function sitemapXml(pages: { path: string; updated: string }[]): string {
  const urls = pages
    .map((p) => `  <url><loc>${SITE_URL}${p.path}</loc><lastmod>${p.updated}</lastmod></url>\n`)
    .join('');
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}</urlset>\n`;
}

/** The Vietnamese address of an English page: / → /vi, /guides → /vi/guides. */
function viPath(path: string): string {
  return path === '/' ? '/vi' : `/vi${path}`;
}

/** Where the prerendered file of `path` goes: / is pages/index.html, /guides/x is pages/guides/x.html. */
function pageFile(outDir: string, path: string): string {
  return path === '/' ? join(outDir, 'pages', 'index.html') : join(outDir, 'pages', `${path.slice(1)}.html`);
}

async function write(file: string, content: string) {
  await mkdir(dirname(file), { recursive: true });
  await writeFile(file, content, 'utf8');
}

export function prerender(): Plugin {
  let config: ResolvedConfig;
  return {
    name: 'revit-downgrade:prerender',
    apply: 'build',
    configResolved(resolved) {
      config = resolved;
    },
    async closeBundle() {
      // The Node build below loads this config again; it must not prerender itself.
      if (config.build.ssr) {
        return;
      }
      const outDir = resolve(config.root, config.build.outDir);
      const ssrDir = resolve(config.root, 'node_modules/.prerender');
      await build({
        configFile: config.configFile,
        mode: config.mode,
        logLevel: 'warn',
        build: {
          ssr: resolve(config.root, 'src/entryPrerender.ts'),
          outDir: ssrDir,
          emptyOutDir: true,
          copyPublicDir: false,
          minify: false,
        },
      });
      const entry = (await import(pathToFileURL(join(ssrDir, 'entryPrerender.js')).href)) as PrerenderEntry;
      const template = await readFile(join(outDir, 'index.html'), 'utf8');
      await rm(join(outDir, 'pages'), { recursive: true, force: true });
      // Every page in English at its address and in Vietnamese under /vi.
      const pages = entry.SITE_PAGES.flatMap((page) => [page, { ...page, path: viPath(page.path) }]);
      for (const page of pages) {
        const rendered = await entry.render(page.path);
        if (rendered.meta?.path !== page.path) {
          throw new Error(`prerender: ${page.path} rendered as ${String(rendered.meta?.path)}`);
        }
        await write(pageFile(outDir, page.path), pageHtml(template, rendered));
      }
      for (const path of [entry.NOT_FOUND_PATH, viPath(entry.NOT_FOUND_PATH)]) {
        await write(pageFile(outDir, path), pageHtml(template, await entry.render(path)));
      }
      await write(join(outDir, 'sitemap.xml'), sitemapXml(pages));
      await rm(ssrDir, { recursive: true, force: true });
      config.logger.info(`prerendered ${String(pages.length)} pages, the 404 pages and sitemap.xml`);
    },
  };
}

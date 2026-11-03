import { GUIDES, guidePath } from '@/shared/catalog/guides';
import { CONVERT_PAGES_UPDATED, PAIRS, pairPath } from '@/shared/catalog/pairs';

/** A page the build prerenders to HTML and lists in the sitemap. `updated` is its last change in substance. */
export interface SitePage {
  path: string;
  updated: string;
}

/** Date the guides list and the "how it works" page last changed in substance. */
const OTHER_PAGES_UPDATED = '2026-11-03';

/** Every page a search engine should find, in sitemap order. */
export const SITE_PAGES: SitePage[] = [
  { path: '/', updated: CONVERT_PAGES_UPDATED },
  { path: '/how-it-works', updated: OTHER_PAGES_UPDATED },
  ...PAIRS.map((pair) => ({ path: pairPath(pair), updated: CONVERT_PAGES_UPDATED })),
  { path: '/guides', updated: OTHER_PAGES_UPDATED },
  ...GUIDES.map((guide) => ({ path: guidePath(guide), updated: guide.updated })),
];

/** Rendered once to the HTML the server returns, with status 404, for an address that is no page. */
export const NOT_FOUND_PATH = '/404';

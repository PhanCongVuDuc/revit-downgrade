import { AUTHOR_NAME, GITHUB_PROFILE_URL, LINKEDIN_URL, REPO_URL } from '@/shared/contact/contact';

import { SITE_NAME, SITE_URL, pageUrl } from './pageMeta';

// Schema.org descriptions of the site, built only from what the pages themselves say.

const AUTHOR_ID = `${SITE_URL}/#author`;
const WEBSITE_ID = `${SITE_URL}/#website`;
const APP_ID = `${SITE_URL}/#app`;

/** The picture every page shares as its link preview. */
export const SITE_IMAGE = `${SITE_URL}/og-image.png`;

export function authorNode() {
  return {
    '@type': 'Person',
    '@id': AUTHOR_ID,
    name: AUTHOR_NAME,
    url: LINKEDIN_URL,
    sameAs: [LINKEDIN_URL, GITHUB_PROFILE_URL],
  };
}

export function websiteNode(description: string, inLanguage = 'en') {
  return {
    '@type': 'WebSite',
    '@id': WEBSITE_ID,
    name: SITE_NAME,
    url: `${SITE_URL}/`,
    description,
    inLanguage,
    publisher: { '@id': AUTHOR_ID },
  };
}

/**
 * The converter as a free web application. `path` is the page it is offered on; `operatingSystem` and
 * `browserRequirements` are in the page's language.
 */
export function webApplicationNode({
  name,
  description,
  path,
  inLanguage = 'en',
  operatingSystem,
  browserRequirements,
}: {
  name: string;
  description: string;
  path: string;
  inLanguage?: string;
  operatingSystem: string;
  browserRequirements: string;
}) {
  return {
    '@type': 'WebApplication',
    '@id': path === '/' ? APP_ID : `${pageUrl(path)}#app`,
    name,
    description,
    url: pageUrl(path),
    inLanguage,
    image: SITE_IMAGE,
    applicationCategory: 'DesignApplication',
    operatingSystem,
    browserRequirements,
    isAccessibleForFree: true,
    isPartOf: { '@id': WEBSITE_ID },
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    author: { '@id': AUTHOR_ID },
    sameAs: REPO_URL,
  };
}

export interface Crumb {
  name: string;
  path: string;
}

export function breadcrumbNode(crumbs: Crumb[]) {
  return {
    '@type': 'BreadcrumbList',
    itemListElement: crumbs.map((crumb, i) => ({
      '@type': 'ListItem',
      position: i + 1,
      name: crumb.name,
      item: pageUrl(crumb.path),
    })),
  };
}

export function articleNode({
  headline,
  description,
  path,
  published,
  modified,
  inLanguage = 'en',
}: {
  headline: string;
  description: string;
  path: string;
  published: string;
  modified: string;
  inLanguage?: string;
}) {
  const url = pageUrl(path);
  return {
    '@type': 'Article',
    '@id': `${url}#article`,
    headline,
    description,
    url,
    image: SITE_IMAGE,
    mainEntityOfPage: { '@type': 'WebPage', '@id': url },
    datePublished: published,
    dateModified: modified,
    inLanguage,
    author: { '@id': AUTHOR_ID },
    publisher: { '@id': AUTHOR_ID },
    isPartOf: { '@id': WEBSITE_ID },
  };
}

/** A page that is neither the app nor an article: `CollectionPage` for a list of pages, `WebPage` otherwise. */
export function webPageNode({
  type,
  name,
  description,
  path,
  inLanguage = 'en',
}: {
  type: 'WebPage' | 'CollectionPage';
  name: string;
  description: string;
  path: string;
  inLanguage?: string;
}) {
  return {
    '@type': type,
    '@id': pageUrl(path),
    name,
    description,
    url: pageUrl(path),
    inLanguage,
    isPartOf: { '@id': WEBSITE_ID },
    author: { '@id': AUTHOR_ID },
  };
}

/** One JSON-LD document holding several linked nodes. */
export function graph(...nodes: object[]): object {
  return { '@context': 'https://schema.org', '@graph': nodes };
}

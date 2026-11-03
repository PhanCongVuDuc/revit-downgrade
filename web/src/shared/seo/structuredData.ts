import { AUTHOR_NAME, GITHUB_PROFILE_URL, LINKEDIN_URL, REPO_URL } from '@/shared/contact/contact';

import { SITE_NAME, SITE_URL, pageUrl } from './pageMeta';

// Schema.org descriptions of the site, built only from what the pages themselves say.

const AUTHOR_ID = `${SITE_URL}/#author`;
const WEBSITE_ID = `${SITE_URL}/#website`;
const APP_ID = `${SITE_URL}/#app`;

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

/** The converter as a free web application. `path` is the page it is offered on. */
export function webApplicationNode({ name, description, path }: { name: string; description: string; path: string }) {
  return {
    '@type': 'WebApplication',
    '@id': path === '/' ? APP_ID : `${pageUrl(path)}#app`,
    name,
    description,
    url: pageUrl(path),
    applicationCategory: 'DesignApplication',
    operatingSystem: 'Any (runs in a web browser)',
    browserRequirements: 'Requires JavaScript.',
    isAccessibleForFree: true,
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
  return {
    '@type': 'Article',
    headline,
    description,
    url: pageUrl(path),
    mainEntityOfPage: pageUrl(path),
    datePublished: published,
    dateModified: modified,
    inLanguage,
    author: { '@id': AUTHOR_ID },
    publisher: { '@id': AUTHOR_ID },
    isPartOf: { '@id': WEBSITE_ID },
  };
}

/** One JSON-LD document holding several linked nodes. */
export function graph(...nodes: object[]): object {
  return { '@context': 'https://schema.org', '@graph': nodes };
}

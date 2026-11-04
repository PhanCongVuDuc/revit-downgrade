/**
 * Articles under /guides/<slug>; `key` is where their text sits in the locale files. `published` and `updated` are the
 * dates shown on the article (ISO yyyy-mm-dd): change `updated` whenever its text changes in substance.
 */
export interface Guide {
  slug: string;
  key: string;
  published: string;
  updated: string;
}

export const GUIDES: readonly [Guide, ...Guide[]] = [
  { slug: 'revit-save-as-older-version', key: 'saveAsOlder', published: '2026-10-28', updated: '2026-11-03' },
  { slug: 'open-newer-revit-file-in-older-revit', key: 'openInOlder', published: '2026-10-28', updated: '2026-11-03' },
  { slug: 'revit-version-compatibility', key: 'compatibility', published: '2026-10-28', updated: '2026-11-04' },
  { slug: 'what-changes-when-downgrading-revit', key: 'whatChanges', published: '2026-10-28', updated: '2026-11-04' },
];

export function guideBySlug(slug: string): Guide | undefined {
  return GUIDES.find((g) => g.slug === slug);
}

export function guidePath(guide: Guide): string {
  return `/guides/${guide.slug}`;
}

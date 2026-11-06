/** Oldest and newest Revit versions the engine converts between. */
export const OLDEST = 2022;
export const NEWEST = 2027;

export interface Pair {
  from: number;
  to: number;
}

/** Every downgrade with a page of its own, newest source first. */
export const PAIRS: Pair[] = Array.from({ length: NEWEST - OLDEST }, (_, i) => NEWEST - i).flatMap((from) =>
  Array.from({ length: from - OLDEST }, (_, j) => ({ from, to: from - 1 - j })),
);

export function isPair(from: number, to: number): boolean {
  return PAIRS.some((p) => p.from === from && p.to === to);
}

/** Path of a pair's page, e.g. /revit-2026-to-2024. */
export function pairPath({ from, to }: Pair): string {
  return `/revit-${String(from)}-to-${String(to)}`;
}

/** The versions a file passes through, one step at a time: 2026 → 2024 is [2026, 2025, 2024]. */
export function pairVersions({ from, to }: Pair): number[] {
  return Array.from({ length: from - to + 1 }, (_, i) => from - i);
}

/** One step down from each version, and the two-version steps people search for most: the guides link to these. */
export const GUIDE_PAIRS: Pair[] = [
  ...PAIRS.filter((p) => p.from - p.to === 1),
  { from: 2027, to: 2025 },
  { from: 2026, to: 2024 },
  { from: 2025, to: 2023 },
];

/** A pair written as the locale files write it, `2026-2024`; undefined when it is not a page of the site. */
export function pairFromKey(key: string): Pair | undefined {
  const [from, to] = key.split('-').map(Number);
  return from !== undefined && to !== undefined && isPair(from, to) ? { from, to } : undefined;
}

/** Date the pair pages and the home page last changed in substance (ISO yyyy-mm-dd), for the sitemap. */
export const CONVERT_PAGES_UPDATED = '2026-11-06';

import { describe, expect, it } from 'vitest';

import { PAIRS, isPair, pairFromKey, pairPath } from './pairs';

describe('pairFromKey', () => {
  it('reads a pair the way the locale files write it', () => {
    expect(pairFromKey('2026-2024')).toEqual({ from: 2026, to: 2024 });
  });

  it('knows no pair the site has no page for', () => {
    expect(pairFromKey('2024-2026')).toBeUndefined();
    expect(pairFromKey('2021-2020')).toBeUndefined();
    expect(pairFromKey('')).toBeUndefined();
  });
});

describe('pairs', () => {
  it('lists every downgrade from Revit 2027 down to 2022, newest first', () => {
    expect(PAIRS).toHaveLength(15);
    expect(PAIRS[0]).toEqual({ from: 2027, to: 2026 });
    expect(PAIRS.at(-1)).toEqual({ from: 2023, to: 2022 });
  });

  it('knows which pairs exist and where their pages are', () => {
    expect(isPair(2026, 2024)).toBe(true);
    expect(isPair(2022, 2026)).toBe(false);
    expect(pairPath({ from: 2026, to: 2024 })).toBe('/revit-2026-to-2024');
  });
});

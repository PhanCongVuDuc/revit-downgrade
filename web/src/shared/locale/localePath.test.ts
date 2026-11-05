import { describe, expect, it } from 'vitest';

import { basePath, localizedPath, pathLocale } from './localePath';

describe('localePath', () => {
  it('reads the language from the address', () => {
    expect(pathLocale('/')).toBe('en');
    expect(pathLocale('/guides')).toBe('en');
    expect(pathLocale('/vi')).toBe('vi');
    expect(pathLocale('/zh/guides')).toBe('zh');
    expect(pathLocale('/hi/revit-2026-to-2024')).toBe('hi');
  });

  it('does not take a page that only starts like a language for one', () => {
    expect(pathLocale('/hindi')).toBe('en');
    expect(pathLocale('/zhang/x')).toBe('en');
  });

  it('removes the prefix', () => {
    expect(basePath('/zh')).toBe('/');
    expect(basePath('/hi/guides')).toBe('/guides');
    expect(basePath('/guides')).toBe('/guides');
  });

  it('builds the address in each language', () => {
    expect(localizedPath('/', 'zh')).toBe('/zh');
    expect(localizedPath('/guides', 'hi')).toBe('/hi/guides');
    expect(localizedPath('/vi/guides', 'zh')).toBe('/zh/guides');
    expect(localizedPath('/zh/guides', 'en')).toBe('/guides');
    expect(localizedPath('/guides', 'fr')).toBe('/guides');
  });
});

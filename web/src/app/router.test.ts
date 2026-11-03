import { render, screen, within } from '@testing-library/vue';
import { createPinia } from 'pinia';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory } from 'vue-router';

import App from '@/App.vue';
import aboutEn from '@/features/about/locales/en.json';
import convertEn from '@/features/convert/locales/en.json';

import { currentHeadMeta } from '@/shared/seo/pageMeta';

import { createAppI18n } from './i18n';
import { installLocaleRouting } from './localeRouting';
import { createAppRouter } from './router';

// The convert view asks the server about a remembered or linked job and for the usage totals; here the server knows
// nothing.
vi.mock('@/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api')>();
  return {
    ...actual,
    getJob: vi.fn().mockRejectedValue(new actual.ApiError(404, 'not_found', 'unknown')),
    getStats: vi.fn().mockResolvedValue(null),
  };
});

async function open(path: string) {
  const router = createAppRouter(createMemoryHistory());
  const i18n = createAppI18n('en-US', { getItem: () => null, setItem: () => undefined });
  installLocaleRouting(router, i18n);
  await router.push(path);
  await router.isReady();
  render(App, { global: { plugins: [createPinia(), router, i18n] } });
  return router;
}

describe('router', () => {
  it('renders the convert view at /', async () => {
    const router = await open('/');
    expect(router.currentRoute.value.name).toBe('convert');
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(convertEn.convert.title);
  });

  it('renders the about view at /how-it-works', async () => {
    const router = await open('/how-it-works');
    expect(router.currentRoute.value.name).toBe('about');
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(aboutEn.about.title);
  });

  it('renders the convert view for /job/:id', async () => {
    const router = await open('/job/0123456789abcdef0123456789abcdef');
    expect(router.currentRoute.value.name).toBe('job');
    expect(router.currentRoute.value.params.id).toBe('0123456789abcdef0123456789abcdef');
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(convertEn.convert.title);
  });

  it('renders a page of its own for each downgrade, titled for search', async () => {
    const router = await open('/revit-2026-to-2024');
    expect(router.currentRoute.value.name).toBe('pair');
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Downgrade Revit 2026 to Revit 2024');
    expect(document.title).toBe('Downgrade Revit 2026 to 2024 Online, Free | Revit Downgrade');
  });

  it('gives each downgrade page text of its own and leaves itself out of its links', async () => {
    await open('/revit-2026-to-2024');
    expect(screen.getByText(/The file goes down 2 versions, one at a time: 2026 → 2025 → 2024\./)).toBeInTheDocument();
    expect(screen.getByText('How do I open a Revit 2026 file in Revit 2024?')).toBeInTheDocument();
    expect(screen.queryByText(/a Revit 2026 model can be opened in Revit 2024/)).toBeNull();
    const others = within(screen.getByRole('navigation', { name: 'Other conversions' }));
    expect(others.queryByRole('link', { name: 'Revit 2026 to 2024' })).toBeNull();
    expect(others.getByRole('link', { name: 'Revit 2023 to 2022' })).toHaveAttribute('href', '/revit-2023-to-2022');
    expect(currentHeadMeta().canonical).toBe('https://revitdowngrade.com/revit-2026-to-2024');
  });

  it('says a one-version downgrade is a single step', async () => {
    await open('/revit-2023-to-2022');
    expect(screen.getByText(/This is a one-version step, 2023 → 2022\./)).toBeInTheDocument();
  });

  it('shows "page not found" for a downgrade the engine does not do, at the address asked for', async () => {
    const router = await open('/revit-2023-to-2026');
    expect(router.currentRoute.value.name).toBe('notFound');
    expect(router.currentRoute.value.path).toBe('/revit-2023-to-2026');
    expect(await screen.findByRole('heading', { level: 1 })).toHaveTextContent('Page not found');
    expect(currentHeadMeta().robots).toBe('noindex');
  });

  it('lists the guides and opens one', async () => {
    const router = await open('/guides');
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Revit downgrade guides');
    await router.push('/guides/revit-version-compatibility');
    expect(await screen.findByRole('heading', { level: 1 })).toHaveTextContent(
      'Revit version compatibility: which Revit opens which file',
    );
    const row = screen.getByRole('row', { name: /^Revit 2023 / });
    expect(row).toHaveTextContent('Revit 2023 and earlier');
    expect(row).toHaveTextContent('Revit 2024 to 2027');
    expect(document.title).toBe('Revit version compatibility: which Revit opens which file | Revit Downgrade');
    expect(currentHeadMeta().canonical).toBe('https://revitdowngrade.com/guides/revit-version-compatibility');
    const jsonLd = currentHeadMeta().jsonLd.join(' ');
    expect(jsonLd).toContain('"@type":"Article"');
    expect(jsonLd).toContain('"datePublished":"2026-10-28"');
  });

  it('shows "page not found" for an unknown guide', async () => {
    const router = await open('/guides/no-such-guide');
    expect(router.currentRoute.value.name).toBe('notFound');
    expect(await screen.findByRole('heading', { level: 1 })).toHaveTextContent('Page not found');
  });

  it('shows "page not found" for an unknown address, with links back', async () => {
    const router = await open('/no/such/page');
    expect(router.currentRoute.value.name).toBe('notFound');
    expect(router.currentRoute.value.path).toBe('/no/such/page');
    expect(await screen.findByRole('link', { name: 'Downgrade a Revit file' })).toHaveAttribute('href', '/');
  });
});

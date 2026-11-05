// What a search engine or link preview gets from the real server before any JavaScript runs.
import { expect, test } from '@playwright/test';

import { revitFile } from './fixtures';

test('each_page_is_served_with_its_own_head_and_content', async ({ request }) => {
  const resp = await request.get('/revit-2026-to-2024', { maxRedirects: 0 });
  expect(resp.status()).toBe(200);
  const html = await resp.text();
  expect(html).toContain('<title>Downgrade Revit 2026 to 2024 Online, Free | Revit Downgrade</title>');
  expect(html).toContain('<link rel="canonical" href="https://revitdowngrade.com/revit-2026-to-2024" />');
  expect(html).toContain('"@type":"WebApplication"');
  expect(html).toContain('The file goes down 2 versions, one at a time: 2026 → 2025 → 2024.');
  expect(html).toContain('Revit 2026 → Revit 2025');
  expect(html).toContain('Revit 2025 → Revit 2024');
  expect(html).not.toContain('Revit 2024 → Revit 2023');
  expect(html).toContain(
    'href="https://help.autodesk.com/view/RVT/2026/ENU/?guid=GUID-C81929D7-02CB-4BF7-A637-9B98EC9EB38B"',
  );
  expect(html).toContain(
    '<link rel="alternate" hreflang="vi" href="https://revitdowngrade.com/vi/revit-2026-to-2024" />',
  );

  const guide = await (await request.get('/guides/revit-version-compatibility')).text();
  expect(guide).toContain(
    '<link rel="canonical" href="https://revitdowngrade.com/guides/revit-version-compatibility" />',
  );
  expect(guide).toContain('"@type":"Article"');
  expect(guide).toContain('Revit 2024 to 2027');
});

test('unknown_addresses_are_404_and_kept_out_of_search', async ({ request }) => {
  for (const path of ['/no-such-page', '/revit-2022-to-2021', '/guides/nope', '/favicon.png']) {
    const resp = await request.get(path, { maxRedirects: 0 });
    expect(resp.status(), path).toBe(404);
    const html = await resp.text();
    expect(html, path).toContain('<meta name="robots" content="noindex" />');
    expect(html, path).toContain('Page not found');
  }
});

test('one_address_per_page', async ({ request }) => {
  const redirects = [
    { path: '/guides/', target: '/guides' },
    { path: '/index.html', target: '/' },
    { path: '/vi/guides/', target: '/vi/guides' },
  ];
  for (const { path, target } of redirects) {
    const resp = await request.get(path, { maxRedirects: 0 });
    expect(resp.status(), path).toBe(308);
    expect(resp.headers().location, path).toBe(target);
  }
});

test('sitemap_lists_every_page_with_its_date', async ({ request }) => {
  const xml = await (await request.get('/sitemap.xml')).text();
  expect(xml.match(/<url>/g)).toHaveLength(100);
  expect(xml).toContain('<loc>https://revitdowngrade.com/vi/guides</loc>');
  expect(xml).toContain('<loc>https://revitdowngrade.com/revit-2023-to-2022</loc><lastmod>');
  for (const file of ['/robots.txt', '/llms.txt', '/favicon.svg', '/favicon.ico', '/og-image.png']) {
    expect((await request.get(file)).status(), file).toBe(200);
  }
});

test('page_still_works_after_the_app_takes_over', async ({ page }) => {
  await page.goto('/guides/revit-version-compatibility');
  await page.getByRole('link', { name: 'Revit 2026 to 2025' }).click();
  await expect(page).toHaveURL(/\/revit-2026-to-2025$/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Downgrade Revit 2026 to Revit 2025');
  await expect(page).toHaveTitle('Downgrade Revit 2026 to 2025 Online, Free | Revit Downgrade');
});

test('guides_are_shared_as_articles_and_link_to_their_conversions', async ({ request }) => {
  const html = await (await request.get('/guides/what-changes-when-downgrading-revit')).text();
  expect(html).toContain('<meta property="og:type" content="article" />');
  expect(html).toContain('<meta property="article:published_time" content="2026-10-28" />');
  expect(html).toContain('"@type":"Article"');
  expect(html).toContain('href="/revit-2026-to-2025"');
  expect(html).toContain('href="/revit-2026-to-2024"');
  const home = await (await request.get('/')).text();
  expect(home).toContain('<meta property="og:type" content="website" />');
  expect(home).toContain('href="/vi"');
});

test('vietnamese_pages_have_their_own_address', async ({ request, page }) => {
  const resp = await request.get('/vi/revit-2026-to-2024');
  expect(resp.status()).toBe(200);
  const html = await resp.text();
  expect(html).toContain('<html lang="vi">');
  expect(html).toContain('<link rel="canonical" href="https://revitdowngrade.com/vi/revit-2026-to-2024" />');
  expect(html).toContain('Hạ file Revit 2026 về Revit 2024');
  const missing = await request.get('/vi/khong-co');
  expect(missing.status()).toBe(404);
  expect(await missing.text()).toContain('Không tìm thấy trang');

  await page.goto('/guides');
  await page.getByTestId('language-menu').click();
  await page.getByRole('link', { name: 'Tiếng Việt', exact: true }).click();
  await expect(page).toHaveURL(/\/vi\/guides$/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Hướng dẫn hạ phiên bản Revit');
  await page.getByTestId('language-menu').click();
  await page.getByRole('link', { name: 'English', exact: true }).click();
  await expect(page).toHaveURL(/\/guides$/);
});

test('chinese_and_hindi_pages_have_their_own_address', async ({ request, page }) => {
  for (const [code, og] of [
    ['zh', 'zh_CN'],
    ['hi', 'hi_IN'],
  ] as const) {
    const resp = await request.get(`/${code}/revit-2026-to-2024`);
    expect(resp.status()).toBe(200);
    const html = await resp.text();
    expect(html).toContain(`<html lang="${code}">`);
    expect(html).toContain(`<meta property="og:locale" content="${og}" />`);
    expect(html).toContain(`<link rel="canonical" href="https://revitdowngrade.com/${code}/revit-2026-to-2024" />`);
    const missing = await request.get(`/${code}/nothing-here`);
    expect(missing.status()).toBe(404);
    expect(await missing.text()).toContain(`<html lang="${code}">`);
  }

  // The text of these languages is fetched when they are opened, in the page and from a link alike.
  await page.goto('/zh/guides');
  await expect(page.getByTestId('language-menu')).toContainText('中文');
  await page.getByTestId('language-menu').click();
  await page.getByRole('link', { name: 'हिन्दी', exact: true }).click();
  await expect(page).toHaveURL(/\/hi\/guides$/);
  await expect(page.getByTestId('language-menu')).toContainText('हिन्दी');
  await expect(page.getByRole('link', { name: 'English', exact: true })).toBeHidden();
});

test('version_checker_reads_files_without_sending_them', async ({ page }) => {
  const sent: string[] = [];
  page.on('request', (request) => {
    if (request.method() !== 'GET') {
      sent.push(request.url());
    }
  });
  await page.goto('/revit-version-checker');
  await page
    .getByLabel('Choose files')
    .setInputFiles([revitFile('model_2026.rvt', 2026), revitFile('old_2022.rvt', 2022)]);
  await expect(page.getByRole('cell', { name: 'Revit 2026', exact: true })).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Revit 2022', exact: true })).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Revit 2026 or newer' })).toBeVisible();
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Download CSV' }).click();
  expect((await download).suggestedFilename()).toBe('revit-versions.csv');
  expect(sent).toEqual([]);
});

test('privacy_and_terms_are_linked_from_every_page', async ({ page }) => {
  await page.goto('/vi');
  await page.getByRole('link', { name: 'Quyền riêng tư' }).click();
  await expect(page).toHaveURL(/\/vi\/privacy$/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Chính sách quyền riêng tư');
  await page.getByRole('link', { name: 'Điều khoản sử dụng' }).click();
  await expect(page).toHaveURL(/\/vi\/terms$/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Điều khoản sử dụng');
});

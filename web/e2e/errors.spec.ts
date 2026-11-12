import { type Page } from '@playwright/test';

import { expect, openApp, revitFile, test } from './fixtures';

async function convert(page: Page, path: string) {
  await openApp(page);
  await page.getByLabel('Revit file', { exact: true }).setInputFiles(path);
  await page.getByRole('button', { name: 'Downgrade' }).click();
}

test('refused_shows_engine_message', async ({ page }) => {
  await convert(page, revitFile('x_refuse_2026.rvt', 2026, 'refuse'));
  await expect(
    page.getByRole('alert').filter({ hasText: 'Not supported yet: Worksharing central files are not supported yet' }),
  ).toBeVisible();
});

test('corrupt_file_is_reported', async ({ page }) => {
  await convert(page, revitFile('x_corrupt_2026.rvt', 2026, 'corrupt'));
  await expect(
    page.getByRole('alert').filter({ hasText: 'The file could not be read. It may be damaged.' }),
  ).toBeVisible();
});

test('failed_file_is_kept_and_can_be_deleted', async ({ page }) => {
  await convert(page, revitFile('x_fail_2026.rvt', 2026, 'fail'));
  await expect(
    page.getByRole('alert').filter({ hasText: 'The conversion failed. The problem has been logged.' }),
  ).toBeVisible();

  await expect(page.getByText(/kept for up to 7 days.*R-[0-9A-Z]{6}\./)).toBeVisible();
  await expect(page.getByRole('checkbox')).toHaveCount(0);

  await page.getByRole('button', { name: 'Delete my file' }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Your file has been deleted from the server.' }),
  ).toBeVisible();
  await expect(page.getByRole('button', { name: 'Delete my file' })).toHaveCount(0);
});

test('unreadable_file_asks_for_a_check_and_can_be_left', async ({ page }) => {
  await convert(page, revitFile('x_unreadable_2026.rvt', 2026, 'unreadable'));
  await expect(
    page.getByRole('alert').filter({ hasText: 'This file looks damaged or incomplete, so it was not converted.' }),
  ).toBeVisible();
  await expect(page.getByText('Does it open in Revit 2026?')).toBeVisible();
  await expect(page.getByText(/report code/)).toHaveCount(0);
  const anyway = page.getByRole('button', { name: 'I opened it in Revit, convert anyway' });
  await expect(anyway).toBeDisabled();

  await page.getByRole('button', { name: 'Choose another file' }).click();
  await expect(page.getByLabel('Revit file', { exact: true })).toBeAttached();
  await expect(page).toHaveURL(/\/$/);
});

test('incomplete_file_says_it_was_not_fully_downloaded', async ({ page }) => {
  await convert(page, revitFile('x_incomplete_2026.rvt', 2026, 'incomplete'));
  await expect(page.getByRole('alert').filter({ hasText: 'not fully downloaded or copied' })).toBeVisible();
});

test('confirmed_file_is_converted_anyway', async ({ page }) => {
  await convert(page, revitFile('x_probe-corrupt_2026.rvt', 2026, 'probe-corrupt'));
  const anyway = page.getByRole('button', { name: 'I opened it in Revit, convert anyway' });
  await expect(anyway).toBeDisabled();
  await page.getByRole('checkbox', { name: 'I opened this file in Revit 2026 and it works' }).check();
  await anyway.click();
  await expect(page.getByRole('link', { name: /Download x_probe-corrupt_2026_R2025\.rvt/ })).toBeVisible();
});

test('confirmed_file_that_still_fails_is_kept', async ({ page }) => {
  await convert(page, revitFile('x_unreadable_2026.rvt', 2026, 'unreadable'));
  await page.getByRole('checkbox', { name: 'I opened this file in Revit 2026 and it works' }).check();
  await page.getByRole('button', { name: 'I opened it in Revit, convert anyway' }).click();
  await expect(
    page.getByRole('alert').filter({ hasText: 'The file could not be read. It may be damaged.' }),
  ).toBeVisible();
  await expect(page.getByText(/kept for up to 7 days.*R-[0-9A-Z]{6}\./)).toBeVisible();
});

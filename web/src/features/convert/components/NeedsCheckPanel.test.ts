import { render, screen } from '@testing-library/vue';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { createI18n } from 'vue-i18n';

import en from '../locales/en.json';
import hi from '../locales/hi.json';
import vi from '../locales/vi.json';
import zh from '../locales/zh.json';
import NeedsCheckPanel from './NeedsCheckPanel.vue';

function show(props: { incomplete?: boolean; confirming?: boolean } = {}, locale = 'en') {
  const i18n = createI18n({ legacy: false, locale, messages: { en, vi, zh, hi } });
  return render(NeedsCheckPanel, {
    props: { year: 2026, incomplete: props.incomplete ?? false, confirming: props.confirming ?? false },
    global: { plugins: [i18n] },
  });
}

const anyway = () => screen.getByRole('button', { name: 'I opened it in Revit, convert anyway' });

describe('NeedsCheckPanel', () => {
  it('asks the user to check the copy and Revit, without an error', () => {
    show();
    expect(screen.getByRole('alert')).toHaveTextContent('This file looks damaged or incomplete');
    expect(screen.getByText(/OneDrive, Google Drive, BIM 360 \/ ACC or an email attachment/)).toBeInTheDocument();
    expect(screen.getByText('Does it open in Revit 2026?')).toBeInTheDocument();
    expect(screen.queryByText(/report code/)).not.toBeInTheDocument();
  });

  it('says plainly when the file looks cut short', () => {
    show({ incomplete: true });
    expect(screen.getByRole('alert')).toHaveTextContent('not fully downloaded or copied');
  });

  it('converts anyway only once the user ticks that the file opens', async () => {
    const view = show();
    const user = userEvent.setup();
    const box = screen.getByRole('checkbox', { name: 'I opened this file in Revit 2026 and it works' });
    expect(box).not.toBeChecked();
    expect(anyway()).toBeDisabled();
    await user.click(anyway());
    expect(view.emitted('confirm')).toBeUndefined();
    await user.click(box);
    expect(anyway()).toBeEnabled();
    await user.click(anyway());
    expect(view.emitted('confirm')).toHaveLength(1);
  });

  it('holds the button while the request is on its way', async () => {
    show({ confirming: true });
    await userEvent.setup().click(screen.getByRole('checkbox'));
    expect(screen.getByRole('button', { name: 'Starting…' })).toBeDisabled();
  });

  it('lets the user choose another file', async () => {
    const view = show();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Choose another file' }));
    expect(view.emitted('chooseAnother')).toHaveLength(1);
  });

  it.each(['vi', 'zh', 'hi'])('has every text in %s', (locale) => {
    show({ incomplete: true }, locale);
    expect(screen.getByRole('alert').textContent).not.toContain('convert.');
    expect(screen.getAllByRole('button').every((b) => !b.textContent.includes('convert.'))).toBe(true);
    expect(screen.getByRole('checkbox', { name: /2026/ })).toBeInTheDocument();
  });
});

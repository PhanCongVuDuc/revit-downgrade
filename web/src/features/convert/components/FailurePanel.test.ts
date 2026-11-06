import { render, screen } from '@testing-library/vue';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { createI18n } from 'vue-i18n';

import en from '../locales/en.json';
import type { ReportDeletion } from '../stores/convertStore';
import FailurePanel from './FailurePanel.vue';

function show(props: {
  reportCode?: string;
  reportOwned?: boolean;
  reportDeleted?: boolean;
  reportDeletion?: ReportDeletion;
}) {
  const i18n = createI18n({ legacy: false, locale: 'en', messages: { en } });
  return render(FailurePanel, {
    props: {
      kind: 'failed',
      message: null,
      reportCode: props.reportCode ?? null,
      reportOwned: props.reportOwned ?? false,
      reportDeleted: props.reportDeleted ?? false,
      reportDeletion: props.reportDeletion ?? 'idle',
    },
    global: { plugins: [i18n] },
  });
}

const deleteMine = () => screen.queryByRole('button', { name: 'Delete my file' });
const deleteNow = () => screen.queryByRole('button', { name: 'Delete now' });

describe('FailurePanel', () => {
  it('says the file is kept, with its code, and offers to delete it', async () => {
    const view = show({ reportCode: 'R-ABC234', reportOwned: true });
    expect(screen.getByText(/kept for up to 7 days/)).toHaveTextContent('R-ABC234');
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    expect(deleteNow()).not.toBeInTheDocument();
    await userEvent.setup().click(deleteMine() as HTMLElement);
    expect(view.emitted('deleteReport')).toHaveLength(1);
  });

  it('holds the button while deleting', () => {
    show({ reportCode: 'R-ABC234', reportOwned: true, reportDeletion: 'deleting' });
    expect(screen.getByRole('button', { name: 'Deleting…' })).toBeDisabled();
  });

  it('confirms a deleted file and offers nothing more', () => {
    show({ reportCode: 'R-ABC234', reportOwned: true, reportDeleted: true });
    expect(screen.getByRole('status')).toHaveTextContent('has been deleted');
    expect(screen.queryByText(/kept for up to 7 days/)).not.toBeInTheDocument();
    expect(deleteMine()).not.toBeInTheDocument();
  });

  it('says how to reach the author when the job is gone', () => {
    show({ reportCode: 'R-ABC234', reportOwned: true, reportDeletion: 'gone' });
    const note = screen.getByText(/can no longer delete/);
    expect(note).toHaveTextContent('R-ABC234');
    expect(note).toHaveTextContent('phancongvuduc@gmail.com');
    expect(deleteMine()).not.toBeInTheDocument();
  });

  it('never claims a deletion that failed', () => {
    show({ reportCode: 'R-ABC234', reportOwned: true, reportDeletion: 'failed' });
    const note = screen.getByText(/could not be deleted yet/);
    expect(note).toHaveTextContent('R-ABC234');
    expect(note).toHaveTextContent('phancongvuduc@gmail.com');
    expect(deleteMine()).toBeInTheDocument();
    expect(screen.queryByText(/has been deleted/)).not.toBeInTheDocument();
  });

  it('says an identical file is already kept, and deletes only this job', async () => {
    const view = show({ reportCode: 'R-ABC234' });
    expect(screen.getByText(/identical file is already being kept/)).toHaveTextContent('R-ABC234');
    expect(screen.queryByText(/kept for up to 7 days/)).not.toBeInTheDocument();
    expect(deleteMine()).not.toBeInTheDocument();
    await userEvent.setup().click(deleteNow() as HTMLElement);
    expect(view.emitted('deleteNow')).toHaveLength(1);
  });

  it('claims nothing when no report was made, and still lets the file go', () => {
    show({});
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.queryByText(/kept/)).not.toBeInTheDocument();
    expect(deleteMine()).not.toBeInTheDocument();
    expect(deleteNow()).toBeInTheDocument();
  });
});

import { render, screen } from '@testing-library/vue';
import { describe, expect, it } from 'vitest';

import { createI18n } from 'vue-i18n';

import en from '../locales/en.json';
import FailurePanel from './FailurePanel.vue';

function show(props: { reportCode?: string; alreadyReported?: boolean }) {
  const i18n = createI18n({ legacy: false, locale: 'en', messages: { en } });
  render(FailurePanel, {
    props: {
      kind: 'failed',
      message: null,
      reportCode: props.reportCode ?? null,
      alreadyReported: props.alreadyReported ?? false,
      reportError: null,
      sending: false,
    },
    global: { plugins: [i18n] },
  });
}

describe('FailurePanel', () => {
  it('offers to send a file that is not in a report yet', () => {
    show({});
    expect(screen.getByRole('checkbox')).toBeInTheDocument();
  });

  it('does not ask for a file it already has', () => {
    show({ alreadyReported: true });
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    expect(screen.getByText(/already have this file/)).toBeInTheDocument();
  });
});

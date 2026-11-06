import { describe, expect, it } from 'vitest';

import type { KeyValueStorage } from '@/shared/storage/safeStorage';

import { KEPT_REPORT_MAX_AGE_MS, keptReportRegistry, KEPT_REPORTS_STORAGE_KEY } from './keptReports';

function memory(): KeyValueStorage & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (key) => data.get(key) ?? null,
    setItem: (key, value) => {
      data.set(key, value);
    },
    removeItem: (key) => {
      data.delete(key);
    },
  };
}

const JOB = 'a'.repeat(32);
const OTHER = 'b'.repeat(32);

describe('keptReportRegistry', () => {
  it('finds a code by its job until it is removed', () => {
    const registry = keptReportRegistry(memory());
    registry.add(JOB, 'R-AAAAAA');
    registry.add(OTHER, 'R-BBBBBB');
    expect(registry.find(JOB)).toBe('R-AAAAAA');
    registry.remove(JOB);
    expect(registry.find(JOB)).toBeNull();
    expect(registry.find(OTHER)).toBe('R-BBBBBB');
  });

  it('forgets a code once the server has deleted the report by age', () => {
    let now = 1000;
    const registry = keptReportRegistry(memory(), () => now);
    registry.add(JOB, 'R-AAAAAA');
    now += KEPT_REPORT_MAX_AGE_MS;
    expect(registry.find(JOB)).toBe('R-AAAAAA');
    now += 1;
    expect(registry.find(JOB)).toBeNull();
  });

  it('keeps the first date when the same job is seen again', () => {
    let now = 1000;
    const registry = keptReportRegistry(memory(), () => now);
    registry.add(JOB, 'R-AAAAAA');
    now += KEPT_REPORT_MAX_AGE_MS;
    registry.add(JOB, 'R-AAAAAA');
    now += 1;
    expect(registry.find(JOB)).toBeNull();
  });

  it('never throws when the storage refuses to write', () => {
    const storage = memory();
    storage.setItem = () => {
      throw new Error('quota');
    };
    const registry = keptReportRegistry(storage);
    registry.add(JOB, 'R-AAAAAA');
    registry.remove(JOB);
    expect(registry.find(JOB)).toBeNull();
  });

  it('reads broken storage as empty', () => {
    const storage = memory();
    storage.data.set(KEPT_REPORTS_STORAGE_KEY, '{not json');
    expect(keptReportRegistry(storage).find(JOB)).toBeNull();
    storage.data.set(KEPT_REPORTS_STORAGE_KEY, JSON.stringify({ [JOB]: { code: 5 } }));
    expect(keptReportRegistry(storage).find(JOB)).toBeNull();
  });
});

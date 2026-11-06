import type { KeyValueStorage } from '@/shared/storage/safeStorage';

/** Where this browser remembers the report codes of its failed jobs. */
export const KEPT_REPORTS_STORAGE_KEY = 'revit-downgrade.kept-reports';
/** The server deletes a report at the latest 7 days after it was made. */
export const KEPT_REPORT_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;

interface KeptReport {
  code: string;
  /** Milliseconds since the epoch. */
  keptAt: number;
}

function isKeptReport(value: unknown): value is KeptReport {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const entry = value as Record<string, unknown>;
  return typeof entry.code === 'string' && typeof entry.keptAt === 'number';
}

/**
 * The reports this browser's failed jobs made, by job id. The job on the server goes after 15 minutes or a restart, the
 * report does not: this is what still gives the visitor the code to quote. Blocked or broken storage reads as empty.
 */
export function keptReportRegistry(storage: KeyValueStorage, now: () => number = Date.now) {
  function read(): Record<string, KeptReport> {
    try {
      const parsed: unknown = JSON.parse(storage.getItem(KEPT_REPORTS_STORAGE_KEY) ?? '{}');
      if (typeof parsed !== 'object' || parsed === null) {
        return {};
      }
      const fresh = Object.entries(parsed).filter(
        ([, entry]) => isKeptReport(entry) && now() - entry.keptAt <= KEPT_REPORT_MAX_AGE_MS,
      );
      return Object.fromEntries(fresh);
    } catch {
      return {};
    }
  }

  function write(entries: Record<string, KeptReport>) {
    try {
      storage.setItem(KEPT_REPORTS_STORAGE_KEY, JSON.stringify(entries));
    } catch {
      // Blocked or full storage only costs the code once the job is gone.
    }
  }

  return {
    /** Remembers a job's report; a job seen again keeps its first date. */
    add(jobId: string, code: string) {
      const entries = read();
      if (entries[jobId]?.code !== code) {
        write({ ...entries, [jobId]: { code, keptAt: now() } });
      }
    },
    find(jobId: string): string | null {
      return read()[jobId]?.code ?? null;
    },
    remove(jobId: string) {
      const entries = read();
      if (jobId in entries) {
        write(Object.fromEntries(Object.entries(entries).filter(([id]) => id !== jobId)));
      }
    },
  };
}

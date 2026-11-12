// The server's /api, in camelCase. The wire format (snake_case) is mapped in client.ts.

export type FileKind = 'rvt' | 'rte' | 'rfa' | 'rft';

/** A conversion the server can do: [from year, to year]. */
export type YearPair = [from: number, to: number];

export interface ServerInfo {
  maxBytes: number;
  outputTtlS: number;
  pairs: YearPair[];
  version: string;
}

/** Public usage totals since `since`. */
export interface UsageStats {
  /** The UTC day counting started, YYYY-MM-DD. */
  since: string;
  people: number;
  succeeded: number;
  /** Files still failing, whether or not they are kept. */
  failed: number;
  /** Failing files that are kept and wait for a fix. */
  pending: number;
  fixed: number;
}

export interface CreateJobRequest {
  name: string;
  size: number;
  kind: FileKind;
  year: number;
  target: number;
}

export interface CreatedJob {
  jobId: string;
  uploadUrl: string;
}

/**
 * `needs_check`: the server could not read the file in its first check, so nothing was converted, kept or counted. The
 * user may confirm that it opens in Revit (`confirmJob`) to convert it anyway.
 */
export type JobState =
  'waiting_upload' | 'queued' | 'running' | 'needs_check' | 'done' | 'failed' | 'cancelled' | 'expired';

export type JobErrorKind = 'refused' | 'corrupt' | 'failed' | 'crashed' | 'timed_out' | 'mismatch' | 'disk_full';

export interface JobStatus {
  state: JobState;
  position: number | null;
  elapsedS: number;
  estimateS: number;
  /** RFC 3339 UTC. */
  expiresAt: string | null;
  error: { kind: JobErrorKind; message: string } | null;
  result: { fileName: string; size: number; droppedCount: number } | null;
  reportCode: string | null;
  /** This job's failure made the report, so it can delete it. */
  reportOwned: boolean;
  /** The report this job owned was deleted. */
  reportDeleted: boolean;
  /** An earlier report already holds this same file. */
  alreadyReported: boolean;
  /** In `needs_check`: the file ends in empty data, so it was most likely not fully downloaded or copied. */
  incomplete: boolean;
  /** The user said the file opens in Revit, so it was converted without the first check. */
  confirmedOpensInRevit: boolean;
  /** What the job was created with; the job id is the capability that reveals them. */
  name: string;
  size: number;
  kind: FileKind;
  year: number;
  target: number;
}

export type DropReason = 'no_rule' | 'rule_broken' | 'cascade';

export interface DroppedElement {
  elementId: number;
  class: string;
  reason: DropReason;
  cascadeFrom: number | null;
}

export interface UploadOptions {
  onProgress?: (bytesSent: number, bytesTotal: number) => void;
  signal?: AbortSignal;
}

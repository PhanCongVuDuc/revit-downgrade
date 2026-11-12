import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import * as api from '@/api';
import { ApiError, type JobStatus, type ServerInfo } from '@/api';
import { basicFileInfo, fileOf, makeOle } from '@/shared/revit-file/makeOle.testutil';

import { POLL_INTERVAL_MS, useConvertStore } from './convertStore';
import { UPLOAD_ENTRY_MAX_AGE_MS, UPLOADS_STORAGE_KEY } from './uploadRegistry';

// A shared stand-in for the browser's lock manager: every store in a test (one per tab) sees the same locks.
const lock = vi.hoisted(() => ({
  taken: new Set<string>(),
  /** Grants refused even though the hint said the lock was free (another tab took it in between). */
  refuse: new Set<string>(),
  held: [] as string[],
  released: [] as string[],
}));

vi.mock('@/shared/locks/uploadLock', () => ({
  isUploadLockHeld: (jobId: string) => Promise.resolve(lock.taken.has(jobId)),
  acquireUploadLock: (jobId: string) => {
    if (lock.taken.has(jobId) || lock.refuse.has(jobId)) {
      return Promise.resolve(null);
    }
    lock.taken.add(jobId);
    lock.held.push(jobId);
    return Promise.resolve(() => {
      lock.taken.delete(jobId);
      lock.released.push(jobId);
    });
  },
}));

vi.mock('@/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/api')>();
  return {
    ...actual,
    getInfo: vi.fn(),
    createJob: vi.fn(),
    getJob: vi.fn(),
    getDropped: vi.fn(),
    deleteJob: vi.fn(),
    confirmJob: vi.fn(),
    deleteJobReport: vi.fn(),
    startOpenReport: vi.fn(),
    finishOpenReport: vi.fn(),
    uploadFile: vi.fn(),
  };
});

const mocked = vi.mocked(api);

const JOB = '0123456789abcdef0123456789abcdef';

function serverInfo(pairs: [number, number][]): ServerInfo {
  return { maxBytes: 1_073_741_824, outputTtlS: 3600, pairs, version: '0.1.0' };
}

function status(overrides: Partial<JobStatus>): JobStatus {
  return {
    state: 'queued',
    position: null,
    elapsedS: 0,
    estimateS: 0,
    expiresAt: null,
    error: null,
    result: null,
    reportCode: null,
    reportOwned: false,
    reportDeleted: false,
    alreadyReported: false,
    incomplete: false,
    confirmedOpensInRevit: false,
    name: 'tower.rvt',
    size: 1000,
    kind: 'rvt',
    year: 2026,
    target: 2025,
    ...overrides,
  };
}

const LAST_MODIFIED = 1_790_000_000_000;

function revitFile(year: number, name = 'tower.rvt', lastModified = LAST_MODIFIED): File {
  return new File([fileOf(makeOle([['BasicFileInfo', basicFileInfo(year)]]), name)], name, { lastModified });
}

/** Job ids in this browser's list of unfinished uploads. */
function storedJobs(): string[] {
  const raw = localStorage.getItem(UPLOADS_STORAGE_KEY);
  return raw ? (JSON.parse(raw) as { jobId: string }[]).map((entry) => entry.jobId) : [];
}

/** Puts an unfinished upload of `file` for `jobId` in the list, as an earlier visit would have. */
function storeUpload(file: File, jobId: string, createdAt = Date.now()) {
  const entries = JSON.parse(localStorage.getItem(UPLOADS_STORAGE_KEY) ?? '[]') as unknown[];
  entries.push({
    name: file.name,
    size: file.size,
    lastModified: file.lastModified,
    jobId,
    uploadUrl: `/api/uploads/${jobId}`,
    createdAt,
  });
  localStorage.setItem(UPLOADS_STORAGE_KEY, JSON.stringify(entries));
}

const OTHER = 'ffffffffffffffffffffffffffffffff';

/** Lets pending promise callbacks run; timers stay frozen. */
async function settle() {
  for (let round = 0; round < 20; round += 1) {
    await Promise.resolve();
  }
}

async function readyStore(year = 2026, pairs: [number, number][] = [[2026, 2025]]) {
  mocked.getInfo.mockResolvedValue(serverInfo(pairs));
  const store = useConvertStore();
  await store.chooseFile(revitFile(year));
  return store;
}

const ALL_PAIRS: [number, number][] = [
  [2026, 2025],
  [2026, 2024],
  [2026, 2023],
  [2025, 2024],
];

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  lock.taken.clear();
  lock.refuse.clear();
  lock.held = [];
  lock.released = [];
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  mocked.createJob.mockResolvedValue({ jobId: JOB, uploadUrl: `/api/uploads/${JOB}` });
  mocked.uploadFile.mockResolvedValue(undefined);
  mocked.deleteJob.mockResolvedValue(undefined);
  mocked.getDropped.mockResolvedValue([]);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('choosing a file', () => {
  it('default_target_is_year_below', async () => {
    const store = await readyStore(2026, ALL_PAIRS);
    expect(store.phase).toBe('ready');
    expect(store.info).toEqual({ kind: 'rvt', year: 2026 });
    expect(store.targets).toEqual([2025, 2024, 2023]);
    expect(store.target).toBe(2025);
  });

  it('default_target_falls_back_to_highest_allowed', async () => {
    const store = await readyStore(2026, [
      [2026, 2023],
      [2026, 2024],
    ]);
    expect(store.targets).toEqual([2024, 2023]);
    expect(store.target).toBe(2024);
  });

  it('unsupported_year_sets_error', async () => {
    const store = await readyStore(2022, ALL_PAIRS);
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('unsupported_year');
    expect(store.target).toBeNull();
  });

  it('shows the reader error for a file that is not a Revit model', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    const store = useConvertStore();
    await store.chooseFile(fileOf(new Uint8Array(10), 'plan.dwg'));
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('bad_extension');
  });

  it('lets the user pick only an allowed target', async () => {
    const store = await readyStore(2026, ALL_PAIRS);
    store.setTarget(2023);
    expect(store.target).toBe(2023);
    store.setTarget(2019);
    expect(store.target).toBe(2023);
  });
});

describe('submitting', () => {
  it('submit_twice_creates_one_job', async () => {
    const store = await readyStore();
    mocked.getJob.mockResolvedValue(status({ state: 'queued', position: 2 }));
    await Promise.all([store.submit(), store.submit()]);
    expect(mocked.createJob).toHaveBeenCalledExactlyOnceWith({
      name: 'tower.rvt',
      size: store.file?.size,
      kind: 'rvt',
      year: 2026,
      target: 2025,
    });
    expect(mocked.uploadFile).toHaveBeenCalledOnce();
  });

  it('ignores submit outside the ready phase', async () => {
    const store = useConvertStore();
    await store.submit();
    expect(mocked.createJob).not.toHaveBeenCalled();
  });

  it('uploads, remembers the job, then polls every 2 s until done', async () => {
    const store = await readyStore();
    let finishUpload: () => void = () => undefined;
    mocked.uploadFile.mockImplementation((_file, _url, options) => {
      options?.onProgress?.(50, 100);
      return new Promise((resolve) => {
        finishUpload = () => {
          resolve();
        };
      });
    });
    mocked.getJob
      .mockResolvedValueOnce(status({ state: 'queued', position: 1, estimateS: 40 }))
      .mockResolvedValueOnce(status({ state: 'running', elapsedS: 2, estimateS: 40 }))
      .mockResolvedValueOnce(
        status({
          state: 'done',
          elapsedS: 5,
          expiresAt: '2099-10-25T13:00:00Z',
          result: { fileName: 'tower.rvt', size: 9, droppedCount: 1 },
        }),
      );
    mocked.getDropped.mockResolvedValue([{ elementId: 4, class: 'Wall', reason: 'no_rule', cascadeFrom: null }]);

    const submitted = store.submit();
    await settle();
    expect(store.phase).toBe('uploading');
    expect(store.progress).toBe(0.5);
    expect(store.jobId).toBe(JOB);
    expect(storedJobs()).toEqual([JOB]);
    expect(lock.held).toEqual([JOB]);
    expect(mocked.uploadFile).toHaveBeenCalledWith(store.file, `/api/uploads/${JOB}`, expect.anything());

    finishUpload();
    await submitted;
    await settle();
    expect(store.phase).toBe('queued');
    expect(store.position).toBe(1);
    // The upload is complete: nothing left to continue, and other tabs no longer see it in progress.
    expect(storedJobs()).toEqual([]);
    expect(lock.released).toEqual([JOB]);

    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    await settle();
    expect(store.phase).toBe('running');
    expect(store.elapsedS).toBe(2);

    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    await settle();
    expect(store.phase).toBe('done');
    expect(store.expiresAt).toBe('2099-10-25T13:00:00Z');
    expect(store.dropped).toEqual([{ elementId: 4, class: 'Wall', reason: 'no_rule', cascadeFrom: null }]);

    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 5);
    expect(mocked.getJob).toHaveBeenCalledTimes(3);
  });

  it('shows the server refusal with its retry time', async () => {
    const store = await readyStore();
    mocked.createJob.mockRejectedValue(new ApiError(429, 'hourly_limit', 'Too many.', 1800));
    await store.submit();
    expect(store.phase).toBe('error');
    expect(store.error).toEqual({ code: 'hourly_limit', message: 'Too many.', retryAfterS: 1800 });
    expect(storedJobs()).toEqual([]);
  });

  it('keeps the job after the upload gives up, so it can be resumed later', async () => {
    const store = await readyStore();
    mocked.uploadFile.mockRejectedValue(new ApiError(0, 'upload_failed', 'offline'));
    mocked.getJob.mockResolvedValue(status({ state: 'waiting_upload' }));
    await store.submit();
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('upload_failed');
    expect(store.jobId).toBe(JOB);
    expect(storedJobs()).toEqual([JOB]);

    await store.chooseFile(revitFile(2026, 'other.rvt'));
    expect(mocked.deleteJob).toHaveBeenCalledWith(JOB);
    expect(storedJobs()).toEqual([]);
    expect(store.phase).toBe('ready');
  });

  it('shows disk_full when the server could not store the upload', async () => {
    const store = await readyStore();
    mocked.uploadFile.mockRejectedValue(new ApiError(507, 'storage_error', 'could not store'));
    mocked.getJob.mockResolvedValue(
      status({ state: 'failed', error: { kind: 'disk_full', message: 'The server could not store the file.' } }),
    );
    await store.submit();
    await settle();
    expect(mocked.getJob).toHaveBeenCalledExactlyOnceWith(JOB);
    expect(store.phase).toBe('failed');
    expect(store.error?.code).toBe('disk_full');
  });

  it('follows the job when the upload was closed because it already finished', async () => {
    const store = await readyStore();
    mocked.uploadFile.mockRejectedValue(new ApiError(409, 'upload_closed', 'closed'));
    mocked.getJob.mockResolvedValue(status({ state: 'queued', position: 1 }));
    await store.submit();
    await settle();
    expect(store.phase).toBe('queued');
    expect(store.position).toBe(1);
  });

  it('keeps the upload error when the job is still waiting for its data', async () => {
    const store = await readyStore();
    mocked.uploadFile.mockRejectedValue(new ApiError(0, 'upload_failed', 'offline'));
    mocked.getJob.mockResolvedValue(status({ state: 'waiting_upload' }));
    await store.submit();
    await settle();
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('upload_failed');
  });

  it('warns before leaving the page only while uploading', async () => {
    const store = await readyStore();
    let finishUpload: () => void = () => undefined;
    mocked.uploadFile.mockReturnValue(
      new Promise((resolve) => {
        finishUpload = () => {
          resolve();
        };
      }),
    );
    mocked.getJob.mockResolvedValue(status({ state: 'queued' }));
    const leave = () => {
      const event = new Event('beforeunload', { cancelable: true });
      window.dispatchEvent(event);
      return event.defaultPrevented;
    };
    expect(leave()).toBe(false);
    const submitted = store.submit();
    await settle();
    expect(leave()).toBe(true);
    finishUpload();
    await submitted;
    expect(leave()).toBe(false);
  });
});

describe('job outcomes', () => {
  it('job_gone_shows_restart_notice', async () => {
    const store = await readyStore();
    mocked.getJob.mockRejectedValue(new ApiError(404, 'not_found', 'no such job'));
    await store.submit();
    await settle();
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('server_restarted');
    expect(storedJobs()).toEqual([]);
  });

  it('shows expired on 410 and forgets the job', async () => {
    storeUpload(revitFile(2026), JOB);
    mocked.getJob.mockRejectedValue(new ApiError(410, 'expired', 'gone'));
    const store = useConvertStore();
    await expect(store.resume(JOB)).resolves.toBe(true);
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('expired');
    expect(storedJobs()).toEqual([]);
  });

  it('keeps polling through a network blip', async () => {
    const store = await readyStore();
    mocked.getJob
      .mockResolvedValueOnce(status({ state: 'running' }))
      .mockRejectedValueOnce(new ApiError(0, 'network', 'offline'))
      .mockResolvedValueOnce(status({ state: 'running', elapsedS: 4 }));
    await store.submit();
    await settle();
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    expect(store.phase).toBe('running');
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    await settle();
    expect(store.elapsedS).toBe(4);
  });

  it('shows a failed job with its error kind and the report it made', async () => {
    const store = await readyStore();
    mocked.getJob.mockResolvedValue(
      status({
        state: 'failed',
        error: { kind: 'corrupt', message: 'bad file' },
        reportCode: 'R-ABC123',
        reportOwned: true,
      }),
    );
    await store.submit();
    await settle();
    expect(store.phase).toBe('failed');
    expect(store.error).toEqual({ code: 'corrupt', message: 'bad file', retryAfterS: null });
    expect(store.reportCode).toBe('R-ABC123');
    expect(store.reportOwned).toBe(true);
    expect(store.reportDeleted).toBe(false);
  });

  it('dropped_load_error_is_exposed', async () => {
    mocked.getJob.mockResolvedValue(status({ state: 'done', result: { fileName: 'a.rvt', size: 1, droppedCount: 3 } }));
    mocked.getDropped.mockRejectedValue(new ApiError(0, 'network', 'offline'));
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.phase).toBe('done');
    expect(store.dropped).toBeNull();
    expect(store.droppedError).toBe('network');
  });
});

describe('retained jobs', () => {
  it('retry_upload_resumes_same_job', async () => {
    const store = await readyStore();
    mocked.uploadFile.mockRejectedValueOnce(new ApiError(0, 'upload_failed', 'offline'));
    mocked.getJob
      .mockResolvedValueOnce(status({ state: 'waiting_upload' }))
      .mockResolvedValue(status({ state: 'queued', position: 1 }));
    await store.submit();
    expect(store.error?.code).toBe('upload_failed');
    await store.retryUpload();
    expect(mocked.createJob).toHaveBeenCalledOnce();
    expect(mocked.uploadFile).toHaveBeenCalledTimes(2);
    expect(mocked.uploadFile).toHaveBeenLastCalledWith(store.file, `/api/uploads/${JOB}`, expect.anything());
    expect(store.phase).toBe('queued');
    expect(store.error).toBeNull();
    expect(store.jobId).toBe(JOB);
  });

  it('ignores retryUpload unless the upload gave up', async () => {
    const store = await readyStore();
    await store.retryUpload();
    expect(mocked.uploadFile).not.toHaveBeenCalled();
    expect(store.phase).toBe('ready');
  });

  it('choose_file_keeps_running_job', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    mocked.getJob.mockRejectedValue(new ApiError(400, 'bad_request', 'odd answer'));
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.phase).toBe('error');
    await store.chooseFile(revitFile(2026));
    expect(store.phase).toBe('ready');
    expect(mocked.deleteJob).not.toHaveBeenCalled();
  });
});

describe('resume, cancel and delete', () => {
  it('resume_restores_done_job', async () => {
    mocked.getJob.mockResolvedValue(
      status({
        state: 'done',
        elapsedS: 30,
        estimateS: 25,
        expiresAt: '2026-10-25T14:00:00Z',
        result: { fileName: 'tower.rvt', size: 10, droppedCount: 0 },
      }),
    );
    const store = useConvertStore();
    await expect(store.resume(JOB)).resolves.toBe(true);
    expect(store.phase).toBe('done');
    expect(store.jobId).toBe(JOB);
    expect(store.elapsedS).toBe(30);
    expect(store.expiresAt).toBe('2026-10-25T14:00:00Z');
    expect(store.dropped).toEqual([]);
  });

  it('opening the start page follows nothing, and a malformed id is ignored', async () => {
    storeUpload(revitFile(2026), JOB);
    const store = useConvertStore();
    await expect(store.resume()).resolves.toBe(false);
    await expect(store.resume('not-a-job')).resolves.toBe(false);
    expect(mocked.getJob).not.toHaveBeenCalled();
    expect(store.phase).toBe('pick');
  });

  it('a link to a job still waiting for its upload shows the normal card and touches nothing', async () => {
    storeUpload(revitFile(2026), JOB);
    mocked.getJob.mockResolvedValue(status({ state: 'waiting_upload' }));
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.phase).toBe('pick');
    expect(store.jobId).toBeNull();
    await store.cancel();
    expect(mocked.deleteJob).not.toHaveBeenCalled();
    expect(storedJobs()).toEqual([JOB]);
  });

  it('continues the earlier upload only when its button is pressed', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    storeUpload(revitFile(2026), JOB);
    mocked.getJob
      .mockResolvedValueOnce(status({ state: 'waiting_upload', year: 2026, target: 2024 }))
      .mockResolvedValue(status({ state: 'queued', position: 0 }));
    const store = useConvertStore();
    const same = revitFile(2026);
    await store.chooseFile(same);
    // Choosing the file sends nothing: the card waits for the button.
    expect(store.phase).toBe('ready');
    expect(store.continuing).toBe(true);
    expect(store.target).toBe(2024);
    expect(store.targets).toEqual([2024]);
    expect(store.jobId).toBeNull();
    expect(mocked.uploadFile).not.toHaveBeenCalled();
    expect(lock.held).toEqual([]);

    await store.submit();
    await settle();
    expect(mocked.createJob).not.toHaveBeenCalled();
    expect(mocked.uploadFile).toHaveBeenCalledWith(same, `/api/uploads/${JOB}`, expect.anything());
    expect(store.jobId).toBe(JOB);
    expect(lock.held).toEqual([JOB]);
    expect(store.phase).toBe('queued');
    expect(lock.released).toEqual([JOB]);
    expect(storedJobs()).toEqual([]);
  });

  it('lock_not_granted_means_other_tab', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    storeUpload(revitFile(2026), JOB);
    mocked.getJob.mockResolvedValue(status({ state: 'waiting_upload' }));
    const store = useConvertStore();
    await store.chooseFile(revitFile(2026));
    expect(store.phase).toBe('ready');
    // Another tab takes the lock between the hint and the press.
    lock.refuse.add(JOB);
    await store.submit();
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('upload_in_other_tab');
    expect(mocked.uploadFile).not.toHaveBeenCalled();
    expect(store.jobId).toBeNull();
    await store.cancel();
    expect(mocked.deleteJob).not.toHaveBeenCalled();
  });

  it('failed_upload_keeps_lock_so_other_tab_cannot_take_over', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    const file = revitFile(2026);
    const tabA = useConvertStore();
    await tabA.chooseFile(file);
    mocked.uploadFile.mockRejectedValueOnce(new ApiError(0, 'upload_failed', 'offline'));
    mocked.getJob.mockResolvedValue(status({ state: 'waiting_upload' }));
    await tabA.submit();
    expect(tabA.error?.code).toBe('upload_failed');
    // Tab A still holds the job's lock while it offers Resume.
    expect(lock.taken.has(JOB)).toBe(true);
    expect(lock.released).toEqual([]);

    // Tab B, same browser, chooses the same file: it may not continue A's job.
    setActivePinia(createPinia());
    const tabB = useConvertStore();
    await tabB.chooseFile(file);
    expect(tabB.phase).toBe('error');
    expect(tabB.error?.code).toBe('upload_in_other_tab');
    expect(mocked.uploadFile).toHaveBeenCalledOnce();

    // Tab A can still resume its own upload.
    mocked.getJob.mockResolvedValue(status({ state: 'queued' }));
    await tabA.retryUpload();
    expect(mocked.uploadFile).toHaveBeenCalledTimes(2);
    expect(tabA.phase).toBe('queued');
    expect(lock.taken.has(JOB)).toBe(false);
  });

  it('cancel_never_deletes_job_continued_elsewhere', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    storeUpload(revitFile(2026), JOB);
    mocked.getJob.mockResolvedValueOnce(status({ state: 'waiting_upload' }));
    const store = useConvertStore();
    await store.chooseFile(revitFile(2026));
    // Another tab finished this upload first: the server closes it and the job is already queued.
    mocked.uploadFile.mockRejectedValueOnce(new ApiError(409, 'upload_closed', 'closed'));
    mocked.getJob.mockResolvedValue(status({ state: 'queued' }));
    await store.submit();
    await settle();
    expect(store.phase).toBe('queued');
    expect(store.ownsJob).toBe(false);
    await store.cancel();
    expect(mocked.deleteJob).not.toHaveBeenCalled();
  });

  it('a new file after a failed upload deletes the old job only with its lock', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    const store = await readyStore();
    mocked.uploadFile.mockRejectedValueOnce(new ApiError(0, 'upload_failed', 'offline'));
    mocked.getJob.mockResolvedValue(status({ state: 'waiting_upload' }));
    await store.submit();
    expect(lock.taken.has(JOB)).toBe(true);
    await store.chooseFile(revitFile(2026, 'other.rvt'));
    expect(mocked.deleteJob).toHaveBeenCalledWith(JOB);
    expect(lock.taken.has(JOB)).toBe(false);
  });

  it('prunes old entries as soon as the store starts', () => {
    storeUpload(revitFile(2026), JOB, Date.now() - UPLOAD_ENTRY_MAX_AGE_MS - 1);
    useConvertStore();
    expect(localStorage.getItem(UPLOADS_STORAGE_KEY)).toBeNull();
  });

  it('starts a new job for a different file, or the same name with another date', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    storeUpload(revitFile(2026), JOB);
    const store = useConvertStore();
    await store.chooseFile(revitFile(2026, 'other.rvt'));
    expect(store.phase).toBe('ready');
    await store.chooseFile(revitFile(2026, 'tower.rvt', LAST_MODIFIED + 1));
    expect(store.phase).toBe('ready');
    expect(mocked.getJob).not.toHaveBeenCalled();
    expect(storedJobs()).toEqual([JOB]);
  });

  it('says so when another tab is uploading the same file, and deletes nothing', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    storeUpload(revitFile(2026), JOB);
    lock.taken.add(JOB);
    mocked.getJob.mockResolvedValue(status({ state: 'waiting_upload' }));
    const store = useConvertStore();
    await store.chooseFile(revitFile(2026));
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('upload_in_other_tab');
    expect(mocked.uploadFile).not.toHaveBeenCalled();
    expect(store.jobId).toBeNull();
    await store.cancel();
    expect(mocked.deleteJob).not.toHaveBeenCalled();
    expect(storedJobs()).toEqual([JOB]);
  });

  it('drops the entry and starts afresh when the earlier job is no longer waiting or is gone', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    storeUpload(revitFile(2026), JOB);
    mocked.getJob.mockResolvedValueOnce(status({ state: 'queued' }));
    const store = useConvertStore();
    await store.chooseFile(revitFile(2026));
    expect(store.phase).toBe('ready');
    expect(storedJobs()).toEqual([]);

    storeUpload(revitFile(2026), OTHER);
    mocked.getJob.mockRejectedValueOnce(new ApiError(404, 'not_found', 'gone'));
    await store.chooseFile(revitFile(2026));
    expect(store.phase).toBe('ready');
    expect(storedJobs()).toEqual([]);
    expect(mocked.deleteJob).not.toHaveBeenCalled();
  });

  it('keeps the entry when the server cannot be asked', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    storeUpload(revitFile(2026), JOB);
    mocked.getJob.mockRejectedValueOnce(new ApiError(0, 'network', 'offline'));
    const store = useConvertStore();
    await store.chooseFile(revitFile(2026));
    expect(store.phase).toBe('ready');
    expect(storedJobs()).toEqual([JOB]);
  });

  it('prunes entries older than 6 hours', async () => {
    mocked.getInfo.mockResolvedValue(serverInfo(ALL_PAIRS));
    storeUpload(revitFile(2026), JOB, Date.now() - UPLOAD_ENTRY_MAX_AGE_MS - 1);
    storeUpload(revitFile(2026, 'b.rvt'), OTHER);
    const store = useConvertStore();
    await store.chooseFile(revitFile(2026));
    expect(mocked.getJob).not.toHaveBeenCalled();
    expect(store.phase).toBe('ready');
    expect(storedJobs()).toEqual([OTHER]);
  });

  it('never deletes a job it only follows from a link', async () => {
    mocked.getJob.mockResolvedValue(status({ state: 'running' }));
    const store = useConvertStore();
    await store.resume(OTHER);
    expect(store.ownsJob).toBe(false);
    await store.cancel();
    expect(mocked.deleteJob).not.toHaveBeenCalled();
    expect(store.phase).toBe('pick');
  });

  it('a reopened running job shows its own versions', async () => {
    mocked.getJob.mockResolvedValue(status({ state: 'running', kind: 'rfa', year: 2026, target: 2024 }));
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.phase).toBe('running');
    expect(store.info).toEqual({ kind: 'rfa', year: 2026 });
    expect(store.target).toBe(2024);
  });

  it('re-checks a done job when its download expires', async () => {
    const expiresAt = new Date(Date.now() + 5000).toISOString();
    mocked.getJob.mockResolvedValueOnce(
      status({ state: 'done', expiresAt, result: { fileName: 'a_R2025.rvt', size: 3, droppedCount: 0 } }),
    );
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.phase).toBe('done');
    mocked.getJob.mockRejectedValue(new ApiError(410, 'expired', 'gone'));
    await vi.advanceTimersByTimeAsync(4000);
    expect(mocked.getJob).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(2000);
    await settle();
    expect(mocked.getJob).toHaveBeenCalledTimes(2);
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('expired');
  });

  it('cancel aborts the upload, deletes the job and returns to pick', async () => {
    const store = await readyStore();
    let signal: AbortSignal | undefined;
    mocked.uploadFile.mockImplementation((_file, _url, options) => {
      signal = options?.signal;
      return new Promise((_resolve, reject) => {
        options?.signal?.addEventListener('abort', () => {
          reject(new ApiError(0, 'aborted', 'cancelled'));
        });
      });
    });
    const submitted = store.submit();
    await settle();
    await store.cancel();
    await submitted;
    expect(signal?.aborted).toBe(true);
    expect(mocked.deleteJob).toHaveBeenCalledWith(JOB);
    expect(store.phase).toBe('pick');
    expect(store.file).toBeNull();
    expect(store.error).toBeNull();
    expect(store.jobId).toBeNull();
    expect(storedJobs()).toEqual([]);
  });

  it('cancel after the upload gave up deletes the waiting job', async () => {
    const store = await readyStore();
    mocked.uploadFile.mockRejectedValue(new ApiError(0, 'upload_failed', 'offline'));
    await store.submit();
    await store.cancel();
    expect(mocked.deleteJob).toHaveBeenCalledWith(JOB);
    expect(store.phase).toBe('pick');
    expect(storedJobs()).toEqual([]);
  });

  it('cancel stops polling', async () => {
    const store = await readyStore();
    mocked.getJob.mockResolvedValue(status({ state: 'queued' }));
    await store.submit();
    await settle();
    await store.cancel();
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3);
    expect(mocked.getJob).toHaveBeenCalledOnce();
    expect(store.phase).toBe('pick');
  });

  it('deleteNow removes a done job and forgets it', async () => {
    mocked.getJob.mockResolvedValue(status({ state: 'done', result: { fileName: 'a.rvt', size: 1, droppedCount: 0 } }));
    const store = useConvertStore();
    await store.resume(JOB);
    await store.deleteNow();
    expect(mocked.deleteJob).toHaveBeenCalledWith(JOB);
    expect(store.phase).toBe('pick');
    expect(store.jobId).toBeNull();
  });

  describe('deleteReport', () => {
    it.each([[new ApiError(404, 'not_found', 'gone')], [new ApiError(410, 'expired', 'gone')]])(
      'still gives the code when a kept job is reopened after it left the server',
      async (gone) => {
        const first = await keptJob();
        first.$reset();
        mocked.getJob.mockRejectedValue(gone);
        const store = useConvertStore();
        await store.resume(JOB);
        expect(store.phase).toBe('failed');
        expect(store.reportCode).toBe('R-ABC123');
        expect(store.reportDeletion).toBe('gone');
      },
    );

    it('does not bring back a report deleted from here', async () => {
      const first = await keptJob();
      await first.deleteReport();
      first.$reset();
      mocked.getJob.mockRejectedValue(new ApiError(410, 'expired', 'gone'));
      const store = useConvertStore();
      await store.resume(JOB);
      expect(store.phase).toBe('error');
      expect(store.error?.code).toBe('expired');
    });

    it('shows a deleted report as deleted when the job is reopened', async () => {
      mocked.getJob.mockResolvedValue(
        status({
          state: 'failed',
          error: { kind: 'failed', message: 'x' },
          reportCode: 'R-ABC123',
          reportOwned: true,
          reportDeleted: true,
        }),
      );
      const store = useConvertStore();
      await store.resume(JOB);
      expect(store.reportDeleted).toBe(true);
    });

    async function keptJob() {
      mocked.getJob.mockResolvedValue(
        status({
          state: 'failed',
          error: { kind: 'failed', message: 'x' },
          reportCode: 'R-ABC123',
          reportOwned: true,
        }),
      );
      const store = useConvertStore();
      await store.resume(JOB);
      return store;
    }

    it('deletes once and says so only after the server has', async () => {
      const store = await keptJob();
      mocked.deleteJobReport.mockResolvedValue(undefined);
      const deleting = store.deleteReport();
      expect(store.reportDeletion).toBe('deleting');
      expect(store.reportDeleted).toBe(false);
      await deleting;
      expect(store.reportDeleted).toBe(true);
      await store.deleteReport();
      expect(mocked.deleteJobReport).toHaveBeenCalledExactlyOnceWith(JOB);
      expect(store.phase).toBe('failed');
    });

    it.each([
      [new ApiError(404, 'not_found', 'gone'), 'gone'],
      [new ApiError(410, 'expired', 'gone'), 'gone'],
      [new ApiError(500, 'report_delete_failed', 'locked'), 'failed'],
      [new ApiError(0, 'network', 'offline'), 'failed'],
    ])('never claims the file is deleted after %s', async (cause, want) => {
      const store = await keptJob();
      mocked.deleteJobReport.mockRejectedValueOnce(cause);
      await store.deleteReport();
      expect(store.reportDeletion).toBe(want);
      expect(store.reportDeleted).toBe(false);
    });

    it('leaves a report this job did not make alone', async () => {
      mocked.getJob.mockResolvedValue(
        status({
          state: 'failed',
          error: { kind: 'failed', message: 'x' },
          reportCode: 'R-ABC123',
          alreadyReported: true,
        }),
      );
      const store = useConvertStore();
      await store.resume(JOB);
      await store.deleteReport();
      expect(mocked.deleteJobReport).not.toHaveBeenCalled();
    });
  });

  it('exposes the result and the download URL of a done job', async () => {
    mocked.getJob.mockResolvedValue(
      status({ state: 'done', result: { fileName: 'a_R2025.rvt', size: 42, droppedCount: 0 } }),
    );
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.result).toEqual({ fileName: 'a_R2025.rvt', size: 42, droppedCount: 0 });
    expect(store.downloadUrl).toBe(api.jobFileUrl(JOB));
    await store.deleteNow();
    expect(store.result).toBeNull();
    expect(store.downloadUrl).toBeNull();
  });

  it('$reset returns to the initial state', async () => {
    const store = await readyStore();
    store.$reset();
    expect(store.phase).toBe('pick');
    expect(store.file).toBeNull();
    expect(store.info).toBeNull();
    expect(store.targets).toEqual([]);
  });
});

describe('reporting a file that does not open in Revit', () => {
  const UPLOAD_URL = `/api/jobs/${JOB}/open-report/upload`;
  const DONE = { state: 'done', result: { fileName: 'tower_R2025.rvt', size: 10, droppedCount: 0 } } as const;

  /** A job this tab uploaded and that is done; the tab still holds its file. */
  async function doneStore() {
    const store = await readyStore();
    mocked.getJob.mockResolvedValue(status({ ...DONE, size: store.file?.size ?? 0 }));
    await store.submit();
    await settle();
    return store;
  }

  beforeEach(() => {
    mocked.startOpenReport.mockResolvedValue(UPLOAD_URL);
    mocked.finishOpenReport.mockResolvedValue('R-7K2M9Q');
  });

  it('sends the file this tab still holds and keeps the report code', async () => {
    const store = await doneStore();
    expect(store.phase).toBe('done');
    expect(store.canReuseFile).toBe(true);
    mocked.uploadFile.mockImplementationOnce((_file, _url, options) => {
      options?.onProgress?.(5, 10);
      return Promise.resolve();
    });
    await expect(store.reportUnopenable('  Revit says the file is corrupt  ')).resolves.toBe(true);
    expect(mocked.startOpenReport).toHaveBeenCalledExactlyOnceWith(
      JOB,
      store.file?.size,
      'Revit says the file is corrupt',
    );
    expect(mocked.uploadFile).toHaveBeenLastCalledWith(store.file, UPLOAD_URL, expect.anything());
    expect(mocked.finishOpenReport).toHaveBeenCalledExactlyOnceWith(JOB);
    expect(store.reportCode).toBe('R-7K2M9Q');
    expect(store.unopenableProgress).toBe(0.5);
    expect(store.unopenableSending).toBe(false);
    expect(store.unopenableError).toBeNull();
    expect(store.phase).toBe('done');
    // Once reported, never again.
    await expect(store.reportUnopenable('')).resolves.toBe(false);
    expect(mocked.startOpenReport).toHaveBeenCalledOnce();
  });

  it('sends no note when the note is blank', async () => {
    const store = await doneStore();
    await store.reportUnopenable('   ');
    expect(mocked.startOpenReport).toHaveBeenCalledExactlyOnceWith(JOB, store.file?.size, undefined);
  });

  it('needs the file again after a reload, then sends the one picked', async () => {
    mocked.getJob.mockResolvedValue(status({ ...DONE, size: 4 }));
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.phase).toBe('done');
    expect(store.canReuseFile).toBe(false);
    await expect(store.reportUnopenable('')).resolves.toBe(false);
    expect(store.unopenableError).toBe('needs_file');
    expect(mocked.startOpenReport).not.toHaveBeenCalled();

    const picked = new File(['abcd'], 'tower.rvt');
    await expect(store.reportUnopenable('', picked)).resolves.toBe(true);
    expect(mocked.startOpenReport).toHaveBeenCalledExactlyOnceWith(JOB, 4, undefined);
    expect(mocked.uploadFile).toHaveBeenLastCalledWith(picked, UPLOAD_URL, expect.anything());
    expect(store.reportCode).toBe('R-7K2M9Q');
    expect(store.unopenableError).toBeNull();
  });

  it('does not reuse a file of another size than the job', async () => {
    const store = await readyStore();
    mocked.getJob.mockResolvedValue(status({ ...DONE, size: (store.file?.size ?? 0) + 1 }));
    await store.submit();
    await settle();
    expect(store.canReuseFile).toBe(false);
  });

  it('exposes the refusal code and lets the user try again', async () => {
    const store = await doneStore();
    mocked.startOpenReport.mockRejectedValueOnce(new ApiError(409, 'size_mismatch', 'Not the original.'));
    await expect(store.reportUnopenable('')).resolves.toBe(false);
    expect(store.unopenableError).toBe('size_mismatch');
    expect(store.unopenableSending).toBe(false);
    expect(store.reportCode).toBeNull();

    mocked.finishOpenReport.mockRejectedValueOnce(new ApiError(409, 'already_reported', 'Sent.'));
    await store.reportUnopenable('');
    expect(store.unopenableError).toBe('already_reported');

    mocked.finishOpenReport.mockRejectedValueOnce(new ApiError(507, 'storage_error', 'Try again.'));
    await store.reportUnopenable('');
    expect(store.unopenableError).toBe('storage_error');

    await expect(store.reportUnopenable('')).resolves.toBe(true);
    expect(store.unopenableError).toBeNull();
    expect(store.reportCode).toBe('R-7K2M9Q');
  });

  it('says expired when the upload finds the job gone', async () => {
    const store = await doneStore();
    mocked.uploadFile.mockRejectedValueOnce(new ApiError(410, 'job_gone', 'gone'));
    mocked.getJob.mockRejectedValueOnce(new ApiError(410, 'expired', 'gone'));
    await expect(store.reportUnopenable('')).resolves.toBe(false);
    expect(store.unopenableError).toBe('expired');

    mocked.uploadFile.mockRejectedValueOnce(new ApiError(404, 'job_gone', 'gone'));
    mocked.getJob.mockRejectedValueOnce(new ApiError(404, 'not_found', 'gone'));
    await store.reportUnopenable('');
    expect(store.unopenableError).toBe('not_found');
  });

  it('shows the report code the server already has after a reload', async () => {
    mocked.getJob.mockResolvedValue(status({ ...DONE, reportCode: 'R-AAAAAA' }));
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.reportCode).toBe('R-AAAAAA');
    await expect(store.reportUnopenable('', new File(['x'], 'a.rvt'))).resolves.toBe(false);
    expect(mocked.startOpenReport).not.toHaveBeenCalled();
  });

  it.each(['upload_closed', 'already_reported'])(
    'shows the code of a report another tab sent first (%s)',
    async (refusal) => {
      const store = await doneStore();
      mocked.uploadFile.mockRejectedValueOnce(new ApiError(409, refusal, 'closed'));
      mocked.getJob.mockResolvedValueOnce(status({ ...DONE, reportCode: 'R-OTHER1' }));
      await expect(store.reportUnopenable('')).resolves.toBe(false);
      expect(store.unopenableError).toBe('already_reported');
      expect(store.reportCode).toBe('R-OTHER1');
    },
  );

  it('refuses to report a failed job', async () => {
    mocked.getJob.mockResolvedValue(status({ state: 'failed', error: { kind: 'corrupt', message: 'bad' } }));
    const store = useConvertStore();
    await store.resume(JOB);
    await expect(store.reportUnopenable('', new File(['x'], 'a.rvt'))).resolves.toBe(false);
    expect(mocked.startOpenReport).not.toHaveBeenCalled();
  });
});

describe('a file the server could not read', () => {
  async function waitingForCheck(incomplete = true) {
    const store = await readyStore();
    mocked.getJob.mockResolvedValueOnce(status({ state: 'needs_check', incomplete }));
    await store.submit();
    await settle();
    return store;
  }

  it('waits for the user and stops polling', async () => {
    const store = await waitingForCheck();
    expect(store.phase).toBe('needs_check');
    expect(store.incomplete).toBe(true);
    expect(store.error).toBeNull();
    expect(store.reportCode).toBeNull();
    const calls = mocked.getJob.mock.calls.length;
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3);
    expect(mocked.getJob.mock.calls).toHaveLength(calls);
  });

  it('converts anyway once confirmed, then follows the job as usual', async () => {
    const store = await waitingForCheck(false);
    expect(store.incomplete).toBe(false);
    mocked.confirmJob.mockResolvedValue(undefined);
    mocked.getJob
      .mockResolvedValueOnce(status({ state: 'running', confirmedOpensInRevit: true }))
      .mockResolvedValueOnce(
        status({
          state: 'failed',
          error: { kind: 'corrupt', message: 'x' },
          reportCode: 'R-ABC234',
          reportOwned: true,
        }),
      );
    await store.confirmOpens();
    expect(mocked.confirmJob).toHaveBeenCalledExactlyOnceWith(JOB);
    expect(store.phase).toBe('running');
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    expect(store.phase).toBe('failed');
    expect(store.reportCode).toBe('R-ABC234');
  });

  it('sends one confirmation at a time', async () => {
    const store = await waitingForCheck();
    let finish: () => void = () => undefined;
    mocked.confirmJob.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = () => {
            resolve();
          };
        }),
    );
    mocked.getJob.mockResolvedValue(status({ state: 'queued' }));
    const first = store.confirmOpens();
    expect(store.confirming).toBe(true);
    await store.confirmOpens();
    finish();
    await first;
    expect(mocked.confirmJob).toHaveBeenCalledOnce();
    expect(store.confirming).toBe(false);
  });

  it('shows why a confirmation was refused', async () => {
    const store = await waitingForCheck();
    mocked.confirmJob.mockRejectedValue(new ApiError(429, 'active_job', 'busy'));
    await store.confirmOpens();
    expect(store.phase).toBe('error');
    expect(store.error?.code).toBe('active_job');
  });

  it('deletes the job when the user chooses another file', async () => {
    const store = await waitingForCheck();
    await store.cancel();
    expect(mocked.deleteJob).toHaveBeenCalledExactlyOnceWith(JOB);
    expect(store.phase).toBe('pick');
    expect(store.jobId).toBeNull();
  });

  it('deletes it too when the job was opened from its link', async () => {
    mocked.getJob.mockResolvedValue(status({ state: 'needs_check' }));
    const store = useConvertStore();
    await store.resume(JOB);
    expect(store.phase).toBe('needs_check');
    expect(store.ownsJob).toBe(false);
    await store.cancel();
    expect(mocked.deleteJob).toHaveBeenCalledExactlyOnceWith(JOB);
  });
});

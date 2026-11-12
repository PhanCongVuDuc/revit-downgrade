//! Background workers that run queued jobs through the conversion engine.

use crate::engine::{DowngradeRequest, Engine, EngineError, Probe, public_message};
use crate::errors::{ErrorLine, ErrorLog, clean_message};
use crate::jobs::{Job, JobStore};
use crate::model::{ErrorKind, time_limit};
use crate::reports::Reports;
use crate::stats::Stats;
use chrono::SubsecRound;
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinHandle;

const MIB: u64 = 1 << 20;
/// `kind` of an error log line for a file the first check could not read (no conversion ran).
pub const NEEDS_CHECK_KIND: &str = "needs_check";

/// Starts `n` workers; each takes queued jobs one at a time, forever. Every job that ends done
/// or failed is counted in `stats`; a failed conversion is also kept in `reports` and written to
/// `errors`.
pub fn spawn_workers(
    n: usize,
    store: Arc<JobStore>,
    engine: Arc<Engine>,
    stats: Arc<Stats>,
    reports: Arc<Reports>,
    errors: Arc<ErrorLog>,
    engine_version: Arc<String>,
) -> Vec<JoinHandle<()>> {
    (0..n)
        .map(|_| {
            let store = store.clone();
            let engine = engine.clone();
            let counts = Counts {
                stats: stats.clone(),
                reports: reports.clone(),
                errors: errors.clone(),
                engine_version: engine_version.clone(),
            };
            tokio::spawn(async move {
                loop {
                    let job = store.next_job().await;
                    let for_panic = job.clone();
                    // Run each job in its own task so a panic cannot end this loop.
                    let run =
                        tokio::spawn(run_job(store.clone(), engine.clone(), counts.clone(), job));
                    if run.await.is_err() {
                        fail(
                            &store,
                            &counts,
                            &for_panic,
                            ErrorKind::Crashed,
                            "The conversion engine stopped unexpectedly.".into(),
                            None,
                            None,
                        )
                        .await;
                    }
                }
            })
        })
        .collect()
}

/// What a finished job is counted and kept in.
#[derive(Clone)]
struct Counts {
    stats: Arc<Stats>,
    reports: Arc<Reports>,
    errors: Arc<ErrorLog>,
    engine_version: Arc<String>,
}

/// Where a failed job's file ended up.
#[derive(Default)]
struct Kept {
    /// The report holding the file.
    code: Option<String>,
    /// This job made that report; otherwise an earlier job's report already holds the same file.
    owned: bool,
}

async fn run_job(store: Arc<JobStore>, engine: Arc<Engine>, counts: Counts, job: Job) {
    run_job_inner(&store, &engine, &counts, &job).await;
    // A job cancelled while running is removed here, once the engine has released its files,
    // whatever the engine reported.
    if job.cancel.is_cancelled() {
        store.remove(&job.id);
    }
}

async fn run_job_inner(store: &JobStore, engine: &Engine, counts: &Counts, job: &Job) {
    if job.cancel.is_cancelled() {
        return;
    }
    // A file the user confirmed opens in Revit already failed this check: it goes straight to the conversion.
    if !job.confirmed_opens_in_revit {
        let probe = match engine.probe(&store.input_path(job), job.cancel.clone()).await {
            Ok(p) => p,
            Err(EngineError::Corrupt { detail, incomplete }) => {
                return needs_check(store, counts, job, detail, incomplete).await;
            }
            Err(e) => return finish_error(store, counts, job, e).await,
        };
        if let Some((kind, message)) = check_probe(job, &probe) {
            return fail(store, counts, job, kind, message.into(), None, None).await;
        }
    }
    convert(store, engine, counts, job).await;
}

/// Why a file the engine read cannot go on to the conversion, if anything.
fn check_probe(job: &Job, probe: &Probe) -> Option<(ErrorKind, &'static str)> {
    if probe.kind != job.kind || probe.year != job.year {
        return Some((
            ErrorKind::Mismatch,
            "The file is not what the browser reported.",
        ));
    }
    if !probe.targets.contains(&job.target) {
        return Some((
            ErrorKind::Refused,
            "This conversion is not supported for this file.",
        ));
    }
    None
}

/// The first check could not read the file: the job waits for the user to check it in Revit. Nothing is converted,
/// kept in a report or counted; only the error log gets a line, of kind `needs_check`, so that it is not taken for a
/// failed conversion.
async fn needs_check(store: &JobStore, counts: &Counts, job: &Job, detail: String, incomplete: bool) {
    if !store.is_running(&job.id) {
        return;
    }
    let detail = Some(detail).filter(|d| !d.is_empty());
    write_error_line(store, counts, job, NEEDS_CHECK_KIND, Some(4), detail, None).await;
    tracing::info!(
        job_id = %job.id,
        incomplete,
        size_mib = job.size.div_ceil(MIB),
        "file needs a check before converting"
    );
    store.finish_needs_check(&job.id, incomplete);
}

async fn convert(store: &JobStore, engine: &Engine, counts: &Counts, job: &Job) {
    let input = store.input_path(job);
    let output = store.output_path(job);
    let log = store.log_path(job);
    let req = DowngradeRequest {
        input: input.clone(),
        output: output.clone(),
        log,
        target: job.target,
        limit: time_limit(job.size, steps(job)),
    };
    match engine.downgrade(req, job.cancel.clone()).await {
        Ok(dropped) => {
            // An engine that reports success must have left a non-empty output file.
            let size = tokio::fs::metadata(&output).await.map_or(0, |m| m.len());
            if size == 0 {
                return fail(
                    store,
                    counts,
                    job,
                    ErrorKind::Failed,
                    "The conversion failed.".into(),
                    Some(0),
                    None,
                )
                .await;
            }
            // The original is about to go: this is the last chance to know which file it was.
            let file = identify(store, &counts.stats, job).await;
            remove_file(&input);
            if store.finish_done(&job.id, dropped, size) {
                record_success(&counts.stats, job, file).await;
            }
        }
        Err(e) => {
            finish_error(store, counts, job, e).await;
        }
    }
}

fn steps(job: &Job) -> u32 {
    u32::from(job.year.saturating_sub(job.target))
}

async fn finish_error(store: &JobStore, counts: &Counts, job: &Job, e: EngineError) {
    // A cancelled job is removed, not counted.
    let Some(kind) = e.kind() else {
        store.remove(&job.id);
        return;
    };
    let exit_code = e.exit_code();
    let detail = e.detail();
    let message = match e {
        EngineError::Refused(m) => public_message(&m, store.data_dir()),
        EngineError::Corrupt { .. } => "The file could not be read.".into(),
        EngineError::Crashed(..) => "The conversion engine stopped unexpectedly.".into(),
        EngineError::TimedOut => "The conversion took too long and was stopped.".into(),
        _ => "The conversion failed.".into(),
    };
    fail(store, counts, job, kind, message, exit_code, detail).await;
}

/// Shared failure exit: drops any partial output, keeps the file of a conversion that went wrong
/// in a report, writes the error log, logs the failure and marks the job failed. `message` is what
/// the user reads, `detail` what the engine really said. The log line names the job and its
/// report, never the file or the engine's message.
async fn fail(
    store: &JobStore,
    counts: &Counts,
    job: &Job,
    kind: ErrorKind,
    message: String,
    exit_code: Option<i32>,
    detail: Option<String>,
) {
    remove_file(&store.output_path(job));
    // A target the file cannot go to, or a file that is not what the browser said, is not a
    // conversion that went wrong.
    let conversion_failed = !matches!(kind, ErrorKind::Refused | ErrorKind::Mismatch);
    let file = if conversion_failed {
        identify(store, &counts.stats, job).await
    } else {
        None
    };
    if !store.is_running(&job.id) {
        return;
    }
    // All of this comes before the job shows as failed: the page stops asking once it does, and
    // it shows the report's code right away.
    let kept = if conversion_failed {
        let kept = keep_file(store, counts, job, kind, file).await;
        write_error(store, counts, job, kind, exit_code, detail, &kept).await;
        kept
    } else {
        let (stats, ip) = (counts.stats.clone(), job.ip);
        count(job, move || stats.record_rejected(ip)).await;
        Kept::default()
    };
    tracing::warn!(
        job_id = %job.id,
        kind = kind.as_str(),
        exit_code,
        size_mib = job.size.div_ceil(MIB),
        steps = steps(job),
        report = kept.code.as_deref(),
        "conversion failed"
    );
    if let Some(code) = &kept.code {
        store.set_report_code(&job.id, code.clone());
        if kept.owned {
            store.set_report_owned(&job.id);
        } else {
            store.set_already_reported(&job.id);
        }
    }
    if !store.finish(&job.id, Err((kind, message))) && kept.owned {
        // Cancelled at the last moment: nobody asked to keep a job they cancelled.
        discard_report(counts, job, kept.code).await;
    }
}

/// Counts a failed conversion and keeps its file, unless a report made under this engine build
/// already holds it. A report of an older build is replaced, the new one first so that the file
/// is never without one. When no report can be made the failure is still counted, and the file
/// stays in the job until the job expires.
async fn keep_file(
    store: &JobStore,
    counts: &Counts,
    job: &Job,
    kind: ErrorKind,
    file: Option<String>,
) -> Kept {
    let (stats, reports) = (counts.stats.clone(), counts.reports.clone());
    let version = counts.engine_version.clone();
    let (input, log, now) = (store.input_path(job), store.log_path(job), store.now());
    let owner = job.clone();
    let keep = move || {
        let file = file.as_deref();
        let (ip, target) = (owner.ip, owner.target);
        let kept = {
            let _creating = reports.exclusive();
            // A `FIXED.md` not counted yet comes first: the report is about to be replaced, or
            // this is a fixed file failing again.
            let open = file.map_or_else(Vec::new, |f| stats.open_reports_of(f));
            if let Some(marked) = open.iter().find(|code| reports.has_fixed_marker(code)) {
                stats.mark_fixed(marked);
            }
            if let Some(code) = file.and_then(|f| reports.holding(&stats, f)) {
                stats.record_failure(ip, file, target);
                Kept {
                    code: Some(code),
                    owned: false,
                }
            } else {
                let replaced = file.map_or_else(Vec::new, |f| stats.open_reports_of(f));
                match reports.create(&owner, &input, &log, &version, kind, now) {
                    Ok(code) => {
                        stats.record_failure_kept(ip, file, target, &code, &replaced);
                        for old in &replaced {
                            if let Err(e) = reports.remove(old) {
                                tracing::warn!(error = ?e.kind(), "could not remove a replaced report");
                            }
                        }
                        Kept {
                            code: Some(code),
                            owned: true,
                        }
                    }
                    Err(e) => {
                        tracing::warn!(job_id = %owner.id, error = ?e.kind(), "could not keep a failed file");
                        stats.record_failure(ip, file, target);
                        Kept::default()
                    }
                }
            }
        };
        if kept.owned {
            reports.retain(&stats, now);
        }
        kept
    };
    count(job, keep).await.unwrap_or_default()
}

/// Adds the failure to the error log, with what the engine said made safe to keep.
async fn write_error(
    store: &JobStore,
    counts: &Counts,
    job: &Job,
    kind: ErrorKind,
    exit_code: Option<i32>,
    detail: Option<String>,
    kept: &Kept,
) {
    write_error_line(
        store,
        counts,
        job,
        kind.as_str(),
        exit_code,
        detail,
        kept.code.clone(),
    )
    .await;
}

async fn write_error_line(
    store: &JobStore,
    counts: &Counts,
    job: &Job,
    kind: &str,
    exit_code: Option<i32>,
    detail: Option<String>,
    report: Option<String>,
) {
    let message = detail.map(|d| clean_message(&d, store.data_dir(), &job.name));
    let line = ErrorLine {
        at: store.now().trunc_subsecs(0),
        report,
        kind: kind.to_string(),
        message,
        exit_code,
        engine_version: counts.engine_version.to_string(),
        engine_build: counts.stats.engine_build().to_string(),
        file_kind: job.kind.ext().to_string(),
        year: job.year,
        target: job.target,
        confirmed_opens_in_revit: job.confirmed_opens_in_revit,
    };
    let errors = counts.errors.clone();
    count(job, move || errors.append(&line)).await;
}

/// Removes the report of a job cancelled once it had already failed: the failure stays counted and
/// logged, as for a file deleted right after it failed.
async fn discard_report(counts: &Counts, job: &Job, code: Option<String>) {
    let Some(code) = code else { return };
    let (stats, reports) = (counts.stats.clone(), counts.reports.clone());
    count(job, move || match reports.remove(&code) {
        Ok(()) => stats.forget_report(&code),
        Err(e) => tracing::warn!(error = ?e.kind(), "could not remove a report"),
    })
    .await;
}

/// Which file this job holds, kept on the job for a later report. `None` when it cannot be read:
/// the job is then counted as a file of its own. Hashing touches the disk, so it runs on a
/// blocking thread.
async fn identify(store: &JobStore, stats: &Arc<Stats>, job: &Job) -> Option<String> {
    let (stats, input) = (stats.clone(), store.input_path(job));
    let id = match tokio::task::spawn_blocking(move || stats.file_id(&input)).await {
        Ok(Ok(id)) => id,
        Ok(Err(e)) => {
            tracing::warn!(job_id = %job.id, error = ?e.kind(), "could not identify a file");
            None
        }
        Err(e) => {
            tracing::warn!(job_id = %job.id, error = %e, "could not identify a file");
            None
        }
    };
    store.set_file_id(&job.id, id.clone());
    id
}

async fn record_success(stats: &Arc<Stats>, job: &Job, file: Option<String>) {
    let (stats, ip, target) = (stats.clone(), job.ip, job.target);
    count(job, move || {
        stats.record_success(ip, file.as_deref(), target);
    })
    .await;
}

/// Runs a change to the statistics. Writing them touches the disk, so it runs on a blocking
/// thread; the worker waits for it before taking the next job, so a shutdown never drops a count
/// that has not started. Its result never changes the job's.
async fn count<T: Send + 'static>(job: &Job, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    match tokio::task::spawn_blocking(f).await {
        Ok(value) => Some(value),
        Err(e) => {
            tracing::warn!(job_id = %job.id, error = %e, "could not count a finished conversion");
            None
        }
    }
}

/// Deletes a file; a missing file is fine, other errors are logged by kind only.
fn remove_file(path: &Path) {
    if let Err(e) = std::fs::remove_file(path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = ?e.kind(), "could not delete a job file");
    }
}

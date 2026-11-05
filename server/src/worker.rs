//! Background workers that run queued jobs through the conversion engine.

use crate::engine::{DowngradeRequest, Engine, EngineError, public_message};
use crate::jobs::{Job, JobStore};
use crate::model::{ErrorKind, time_limit};
use crate::reports::Reports;
use crate::stats::Stats;
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinHandle;

const MIB: u64 = 1 << 20;

/// Starts `n` workers; each takes queued jobs one at a time, forever. Every job that ends done
/// or failed is counted in `stats`.
pub fn spawn_workers(
    n: usize,
    store: Arc<JobStore>,
    engine: Arc<Engine>,
    stats: Arc<Stats>,
    reports: Arc<Reports>,
) -> Vec<JoinHandle<()>> {
    (0..n)
        .map(|_| {
            let store = store.clone();
            let engine = engine.clone();
            let counts = Counts {
                stats: stats.clone(),
                reports: reports.clone(),
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
                        )
                        .await;
                    }
                }
            })
        })
        .collect()
}

/// What a finished job is counted in.
#[derive(Clone)]
struct Counts {
    stats: Arc<Stats>,
    reports: Arc<Reports>,
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
    let input = store.input_path(job);
    let output = store.output_path(job);
    let log = store.log_path(job);

    let probe = match engine.probe(&input, job.cancel.clone()).await {
        Ok(p) => p,
        Err(e) => return finish_error(store, counts, job, e).await,
    };
    if probe.kind != job.kind || probe.year != job.year {
        return fail(
            store,
            counts,
            job,
            ErrorKind::Mismatch,
            "The file is not what the browser reported.".into(),
            None,
        )
        .await;
    }
    if !probe.targets.contains(&job.target) {
        return fail(
            store,
            counts,
            job,
            ErrorKind::Refused,
            "This conversion is not supported for this file.".into(),
            None,
        )
        .await;
    }

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
    let message = match e {
        EngineError::Refused(m) => public_message(&m, store.data_dir()),
        EngineError::Corrupt => "The file could not be read.".into(),
        EngineError::Crashed(_) => "The conversion engine stopped unexpectedly.".into(),
        EngineError::TimedOut => "The conversion took too long and was stopped.".into(),
        _ => "The conversion failed.".into(),
    };
    fail(store, counts, job, kind, message, exit_code).await;
}

/// Shared failure exit: logs the failure, drops any partial output, keeps input and log, marks
/// the job failed. The log line names the job, never the file or the engine's message.
async fn fail(
    store: &JobStore,
    counts: &Counts,
    job: &Job,
    kind: ErrorKind,
    message: String,
    exit_code: Option<i32>,
) {
    tracing::warn!(
        job_id = %job.id,
        kind = kind.as_str(),
        exit_code,
        size_mib = job.size.div_ceil(MIB),
        steps = steps(job),
        "conversion failed"
    );
    remove_file(&store.output_path(job));
    // A target the file cannot go to, or a file that is not what the browser said, is not a
    // conversion that went wrong.
    let conversion_failed = !matches!(kind, ErrorKind::Refused | ErrorKind::Mismatch);
    let file = if conversion_failed {
        identify(store, &counts.stats, job).await
    } else {
        None
    };
    // All of this comes before the job shows as failed: the page stops asking once it does, and
    // a report sent right away finds the file already counted.
    if file
        .as_deref()
        .is_some_and(|f| counts.reports.holds(&counts.stats, f))
    {
        store.set_already_reported(&job.id);
    }
    if !store.is_running(&job.id) {
        return;
    }
    if conversion_failed {
        let (stats, ip, target) = (counts.stats.clone(), job.ip, job.target);
        count(job, move || {
            stats.record_failure(ip, file.as_deref(), target);
        })
        .await;
    } else {
        let (stats, ip) = (counts.stats.clone(), job.ip);
        count(job, move || stats.record_rejected(ip)).await;
    }
    store.finish(&job.id, Err((kind, message)));
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

mod common;

use revit_downgrade_server::AppState;
use revit_downgrade_server::cleanup::wipe_jobs_dir;
use revit_downgrade_server::clock::ManualClock;
use revit_downgrade_server::engine::Engine;
use revit_downgrade_server::jobs::{Job, JobId, NewJob};
use revit_downgrade_server::model::{ErrorKind, FileKind, JobState};
use revit_downgrade_server::reports::Reports;
use revit_downgrade_server::stats::Stats;
use revit_downgrade_server::worker::spawn_workers;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

struct Fixture {
    state: AppState,
    _dir: tempfile::TempDir,
}

fn setup() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
    ));
    Fixture {
        state: common::test_state(dir.path(), clock),
        _dir: dir,
    }
}

/// Creates a job for `ip_last` and simulates the upload: writes the marker file as the input.
fn queue_job_with(
    f: &Fixture,
    pairs: &[(u16, u16)],
    ip_last: u8,
    declared_year: u16,
    target: u16,
    mode: &str,
    content_year: u32,
) -> Job {
    let job = f
        .state
        .store
        .create(
            NewJob {
                name: "House.rvt".into(),
                size: 1024,
                kind: FileKind::Rvt,
                year: declared_year,
                target,
            },
            IpAddr::from([10, 0, 0, ip_last]),
            pairs,
        )
        .unwrap();
    common::write_marker(
        &f.state.store.job_dir(&job),
        "rvt",
        mode,
        Some(content_year),
    );
    f.state.store.mark_uploaded(&job.id);
    job
}

async fn wait_terminal(f: &Fixture, id: &JobId) -> Job {
    for _ in 0..400 {
        match f.state.store.get(id) {
            Some(j)
                if !matches!(
                    j.state,
                    JobState::WaitingUpload | JobState::Queued | JobState::Running
                ) =>
            {
                return j;
            }
            _ => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
    panic!("job did not reach a terminal state");
}

fn queue_job(
    f: &Fixture,
    ip_last: u8,
    declared_year: u16,
    target: u16,
    mode: &str,
    content_year: u32,
) -> Job {
    let pairs = f.state.pairs.clone();
    queue_job_with(
        f,
        &pairs,
        ip_last,
        declared_year,
        target,
        mode,
        content_year,
    )
}

fn start(f: &Fixture, n: usize) {
    spawn_workers(
        n,
        f.state.store.clone(),
        f.state.engine.clone(),
        f.state.stats.clone(),
        f.state.reports.clone(),
        f.state.errors.clone(),
        f.state.engine_version.clone(),
    );
}

#[tokio::test]
async fn ok_job_done_input_deleted_output_kept() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "ok", 2026);
    let done = wait_terminal(&f, &job.id).await;
    assert_eq!(done.state, JobState::Done);
    assert_eq!(
        done.expires_at.unwrap() - done.finished_at.unwrap(),
        chrono::Duration::hours(2)
    );
    assert!(!f.state.store.input_path(&job).exists());
    assert!(f.state.store.output_path(&job).exists());
    assert!(f.state.store.log_path(&job).exists());
}

#[tokio::test]
async fn failed_job_stays_15_min_and_its_input_moves_to_a_report() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    assert_eq!(failed.state, JobState::Failed);
    let err = failed.error.unwrap();
    assert_eq!(err.kind, ErrorKind::Failed);
    assert_eq!(err.message, "The conversion failed.");
    assert!(!f.state.store.input_path(&job).exists());
    assert!(!f.state.store.output_path(&job).exists());
    let left = failed.expires_at.unwrap() - failed.finished_at.unwrap();
    assert_eq!(left.num_minutes(), 15);
}

#[tokio::test]
async fn mismatch_when_probe_year_differs() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2025, 2024, "ok", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    assert_eq!(failed.state, JobState::Failed);
    let err = failed.error.unwrap();
    assert_eq!(err.kind, ErrorKind::Mismatch);
    assert_eq!(err.message, "The file is not what the browser reported.");
    assert!(f.state.store.input_path(&job).exists());
}

#[tokio::test]
async fn refused_message_comes_from_engine() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "refuse", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    let err = failed.error.unwrap();
    assert_eq!(err.kind, ErrorKind::Refused);
    assert!(!err.message.is_empty());
    assert_ne!(err.message, "The conversion failed.");
}

#[tokio::test]
async fn target_not_offered_is_refused() {
    let f = setup();
    start(&f, 1);
    // The store accepts 2026 -> 2021, but the engine does not offer 2021 for this file.
    let job = queue_job_with(&f, &[(2026, 2021)], 1, 2026, 2021, "ok", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    let err = failed.error.unwrap();
    assert_eq!(err.kind, ErrorKind::Refused);
    assert_eq!(
        err.message,
        "This conversion is not supported for this file."
    );
    assert!(!f.state.store.output_path(&job).exists());
}

#[tokio::test]
async fn cancel_running_job_kills_and_deletes() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "hang", 2026);
    let pid_file = f.state.store.output_path(&job).with_extension("rvt.pid");
    let pid = common::wait_for_pid(&pid_file).await;
    f.state.store.cancel(&job.id);
    for _ in 0..200 {
        if f.state.store.get(&job.id).is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(f.state.store.get(&job.id).is_none());
    assert!(!f.state.store.job_dir(&job).exists());
    assert!(!common::process_alive(pid));
}

#[tokio::test]
async fn cancel_queued_job_never_runs() {
    let f = setup();
    // Token cancelled but state left Queued: a worker that takes it must drop it without running the engine.
    let job = queue_job(&f, 1, 2026, 2024, "ok", 2026);
    job.cancel.cancel();
    // The engine path does not exist, so any attempt to start it would fail the job instead of removing it.
    let missing = Arc::new(Engine::new(f._dir.path().join("no-such-engine")));
    spawn_workers(
        1,
        f.state.store.clone(),
        missing,
        f.state.stats.clone(),
        f.state.reports.clone(),
        f.state.errors.clone(),
        f.state.engine_version.clone(),
    );
    for _ in 0..200 {
        if f.state.store.get(&job.id).is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(f.state.store.get(&job.id).is_none());
    assert!(!f.state.store.job_dir(&job).exists());
}

#[tokio::test]
async fn two_workers_run_two_jobs() {
    let f = setup();
    let a = queue_job(&f, 1, 2026, 2024, "hang", 2026);
    let b = queue_job(&f, 2, 2026, 2024, "hang", 2026);
    start(&f, 2);
    // Both hang, so both only get going if two workers run concurrently.
    common::wait_for_pid(&f.state.store.output_path(&a).with_extension("rvt.pid")).await;
    common::wait_for_pid(&f.state.store.output_path(&b).with_extension("rvt.pid")).await;
    assert_eq!(f.state.store.get(&a.id).unwrap().state, JobState::Running);
    assert_eq!(f.state.store.get(&b.id).unwrap().state, JobState::Running);
    f.state.store.cancel(&a.id);
    f.state.store.cancel(&b.id);
}

#[tokio::test]
async fn success_without_output_is_a_failure() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "empty-output", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    assert_eq!(failed.state, JobState::Failed);
    assert_eq!(failed.error.clone().unwrap().kind, ErrorKind::Failed);
    assert!(!f.state.store.output_path(&job).exists());
    assert!(failed.report_code.is_some(), "input kept in a report");
    assert!(!f.state.store.input_path(&job).exists());
}

#[tokio::test]
async fn done_job_records_its_output_size() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "ok", 2026);
    let done = wait_terminal(&f, &job.id).await;
    let on_disk = std::fs::metadata(f.state.store.output_path(&job))
        .unwrap()
        .len();
    assert!(on_disk > 0);
    assert_eq!(done.output_size, on_disk);
}

/// `(failed, pending, fixed)` as `/api/stats` shows them.
fn tally(f: &Fixture) -> (u64, u64, u64) {
    let s = f.state.stats.snapshot().expect("statistics available");
    (s.failed, s.pending, s.fixed)
}

fn report_dirs(f: &Fixture) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(f.state.reports.dir()) else {
        return Vec::new();
    };
    let mut codes: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    codes.sort();
    codes
}

fn meta(f: &Fixture, code: &str) -> serde_json::Value {
    let path = f.state.reports.dir().join(code).join("meta.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Reopens the statistics as if another engine build were running. Call it before `start`.
fn run_on_build(f: &mut Fixture, build: &str) {
    let now = f.state.clock.now();
    let dir = f._dir.path().join("stats");
    f.state.stats = Arc::new(Stats::open(dir, now).with_engine_build(build.into()));
}

/// The stored identity of the marker file of `mode`, whatever job holds it.
fn file_of(f: &Fixture, mode: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = common::write_marker(dir.path(), "rvt", mode, Some(2026));
    f.state.stats.file_id(&path).unwrap().unwrap()
}

/// A report on disk that the statistics follow, made a day ago, as an earlier job would leave it.
fn seed_report(f: &Fixture, code: &str, file: Option<&str>, bytes: usize) {
    let dir = f.state.reports.dir().join(code);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("input.rvt"), vec![0u8; bytes]).unwrap();
    let at = (f.state.clock.now() - chrono::Duration::days(1)).to_rfc3339();
    let text = serde_json::json!({
        "engine_version": "e", "error_kind": "failed", "failed_at": at, "kind": "rvt", "size": bytes,
    });
    std::fs::write(dir.join("meta.json"), text.to_string()).unwrap();
    let ip = IpAddr::from([10, 0, 0, 9]);
    f.state.stats.record_failure_kept(ip, file, 2024, code, &[]);
}

#[tokio::test]
async fn each_failure_kind_is_kept_before_the_job_shows_failed() {
    for (mode, kind) in [
        ("fail", ErrorKind::Failed),
        ("corrupt", ErrorKind::Corrupt),
        ("crash", ErrorKind::Crashed),
    ] {
        let f = setup();
        start(&f, 1);
        let job = queue_job(&f, 1, 2026, 2024, mode, 2026);
        let failed = wait_terminal(&f, &job.id).await;
        // Nothing waits for the job to fail: all of this is there the moment it shows.
        let code = failed.report_code.clone().expect("a report code");
        assert_eq!(failed.state, JobState::Failed);
        assert_eq!(failed.error.unwrap().kind, kind);
        assert!(failed.report_owned, "{mode}");
        assert!(!failed.already_reported, "{mode}");
        assert_eq!(report_dirs(&f), std::slice::from_ref(&code));
        let dir = f.state.reports.dir().join(&code);
        assert!(dir.join("input.rvt").is_file(), "{mode}");
        assert!(!f.state.store.input_path(&job).exists(), "{mode}");
        let m = meta(&f, &code);
        assert_eq!(m["error_kind"], kind.as_str());
        assert_eq!(m["year"], 2026);
        assert_eq!(m["target"], 2024);
        assert_eq!(tally(&f), (1, 1, 0), "{mode}");
    }
}

#[tokio::test]
async fn a_conversion_over_the_time_limit_is_kept() {
    let f = setup();
    let engine = Arc::new(Engine::with_limits(
        common::stub_path(),
        Duration::from_millis(300),
        Duration::from_secs(30),
    ));
    spawn_workers(
        1,
        f.state.store.clone(),
        engine,
        f.state.stats.clone(),
        f.state.reports.clone(),
        f.state.errors.clone(),
        f.state.engine_version.clone(),
    );
    let job = queue_job(&f, 1, 2026, 2024, "probe-hang", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    assert_eq!(failed.error.unwrap().kind, ErrorKind::TimedOut);
    let code = failed.report_code.expect("a report code");
    assert_eq!(meta(&f, &code)["error_kind"], "timed_out");
    assert_eq!(tally(&f), (1, 1, 0));
}

#[tokio::test]
async fn a_failure_with_a_log_keeps_the_log() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    std::fs::write(f.state.store.log_path(&job), b"{}\n").unwrap();
    let failed = wait_terminal(&f, &job.id).await;
    let code = failed.report_code.expect("a report code");
    assert!(f.state.reports.dir().join(code).join("log.jsonl").is_file());
}

#[tokio::test]
async fn refused_and_mismatched_files_are_not_kept() {
    let f = setup();
    start(&f, 1);
    let refused = queue_job(&f, 1, 2026, 2024, "refuse", 2026);
    let mismatched = queue_job(&f, 2, 2025, 2024, "ok", 2026);
    for job in [refused, mismatched] {
        let failed = wait_terminal(&f, &job.id).await;
        assert_eq!(failed.report_code, None);
        assert!(!failed.report_owned);
        assert!(f.state.store.input_path(&job).exists());
    }
    assert!(report_dirs(&f).is_empty());
    assert_eq!(tally(&f), (0, 0, 0));
}

#[tokio::test]
async fn a_cancelled_job_is_not_kept() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "hang", 2026);
    let pid_file = f.state.store.output_path(&job).with_extension("rvt.pid");
    common::wait_for_pid(&pid_file).await;
    f.state.store.cancel(&job.id);
    for _ in 0..200 {
        if f.state.store.get(&job.id).is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(f.state.store.get(&job.id).is_none());
    assert!(report_dirs(&f).is_empty());
    assert_eq!(tally(&f), (0, 0, 0));
}

#[tokio::test]
async fn the_same_file_on_the_same_build_is_kept_once() {
    let f = setup();
    start(&f, 1);
    let first = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let first = wait_terminal(&f, &first.id).await;
    let code = first.report_code.clone().unwrap();

    let again = queue_job(&f, 2, 2026, 2024, "fail", 2026);
    let again_done = wait_terminal(&f, &again.id).await;
    assert_eq!(again_done.report_code.as_deref(), Some(code.as_str()));
    assert!(!again_done.report_owned);
    assert!(again_done.already_reported);
    assert!(
        f.state.store.input_path(&again).exists(),
        "the second copy stays in its job, on the job's time limit"
    );
    assert_eq!(report_dirs(&f), [code]);
    assert_eq!(tally(&f), (1, 1, 0));
}

#[tokio::test]
async fn a_new_build_replaces_the_report_and_changes_no_number() {
    let mut f = setup();
    let file = file_of(&f, "fail");
    run_on_build(&mut f, "old");
    seed_report(&f, "R-OLD000", Some(&file), 10);
    assert_eq!(tally(&f), (1, 1, 0));
    run_on_build(&mut f, "new");
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    let code = failed.report_code.unwrap();
    assert_ne!(code, "R-OLD000");
    assert!(failed.report_owned);
    assert_eq!(report_dirs(&f), [code]);
    assert_eq!(tally(&f), (1, 1, 0));
}

#[tokio::test]
async fn a_fixed_marker_not_counted_yet_is_counted_before_the_file_fails_again() {
    let mut f = setup();
    let file = file_of(&f, "fail");
    run_on_build(&mut f, "old");
    seed_report(&f, "R-OLD000", Some(&file), 10);
    let marker = f.state.reports.dir().join("R-OLD000").join("FIXED.md");
    std::fs::write(&marker, b"checked").unwrap();
    run_on_build(&mut f, "new");
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    assert!(failed.report_owned);
    assert!(marker.is_file(), "a fixed report is not replaced");
    assert_eq!(report_dirs(&f).len(), 2);
    assert_eq!(tally(&f), (1, 1, 1));
}

#[tokio::test]
async fn a_fixed_file_that_fails_again_is_kept_as_a_new_failure() {
    let f = setup();
    start(&f, 1);
    let first = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let code = wait_terminal(&f, &first.id).await.report_code.unwrap();
    f.state.stats.mark_fixed(&code);
    assert_eq!(tally(&f), (0, 0, 1));

    let again = queue_job(&f, 2, 2026, 2024, "fail", 2026);
    let failed = wait_terminal(&f, &again.id).await;
    assert_ne!(failed.report_code.as_deref(), Some(code.as_str()));
    assert!(failed.report_owned);
    assert_eq!(report_dirs(&f).len(), 2);
    assert_eq!(tally(&f), (1, 1, 1));
}

#[tokio::test]
async fn a_report_that_cannot_be_made_leaves_a_plain_failure() {
    let f = setup();
    // A file where the reports directory should go.
    std::fs::write(f.state.reports.dir(), b"").unwrap();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    assert_eq!(failed.state, JobState::Failed);
    assert_eq!(failed.report_code, None);
    assert!(!failed.report_owned);
    assert!(!failed.already_reported);
    assert!(f.state.store.input_path(&job).exists());
    assert_eq!(tally(&f), (1, 0, 0));
}

#[tokio::test]
async fn two_jobs_of_one_file_ending_together_make_one_report() {
    let f = setup();
    let a = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let b = queue_job(&f, 2, 2026, 2024, "fail", 2026);
    start(&f, 2);
    let (a, b) = (
        wait_terminal(&f, &a.id).await,
        wait_terminal(&f, &b.id).await,
    );
    assert_eq!(a.report_code, b.report_code);
    assert!(a.report_owned != b.report_owned);
    assert_eq!(report_dirs(&f).len(), 1);
    assert_eq!(tally(&f), (1, 1, 0));
}

#[tokio::test]
async fn a_file_that_cannot_be_hashed_counts_as_a_file_of_its_own() {
    let f = setup();
    start(&f, 2);
    let mut jobs = Vec::new();
    for ip in 1..=2 {
        // A directory in the place of the input: the engine and the hash cannot read it, a rename
        // can still move it.
        let job = f
            .state
            .store
            .create(
                NewJob {
                    name: "House.rvt".into(),
                    size: 1024,
                    kind: FileKind::Rvt,
                    year: 2026,
                    target: 2024,
                },
                IpAddr::from([10, 0, 0, ip]),
                &f.state.pairs,
            )
            .unwrap();
        std::fs::create_dir(f.state.store.input_path(&job)).unwrap();
        f.state.store.mark_uploaded(&job.id);
        jobs.push(job);
    }
    // The first check cannot read it either: nothing is counted until the user confirms it opens in Revit.
    for job in &jobs {
        assert_eq!(wait_terminal(&f, &job.id).await.state, JobState::NeedsCheck);
    }
    assert_eq!(tally(&f), (0, 0, 0));
    for job in &jobs {
        f.state.store.confirm(&job.id).unwrap();
    }
    for job in &jobs {
        let failed = wait_terminal(&f, &job.id).await;
        assert_eq!(failed.error.unwrap().kind, ErrorKind::Corrupt);
        assert!(failed.report_owned);
    }
    assert_eq!(report_dirs(&f).len(), 2);
    assert_eq!(tally(&f), (2, 2, 0));
}

#[tokio::test]
async fn without_statistics_the_file_is_still_kept() {
    let mut f = setup();
    std::fs::write(f._dir.path().join("stats").join("stats.json"), b"{").unwrap();
    run_on_build(&mut f, "");
    assert_eq!(f.state.stats.snapshot(), None);
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    let code = failed.report_code.expect("a report code");
    assert!(failed.report_owned);
    assert_eq!(report_dirs(&f), [code]);
    assert_eq!(f.state.stats.snapshot(), None);
    assert!(f.state.stats.tracked_reports().is_empty());
}

#[tokio::test]
async fn a_report_survives_a_restart() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let code = wait_terminal(&f, &job.id).await.report_code.unwrap();
    wipe_jobs_dir(f._dir.path()).unwrap();
    assert!(f.state.reports.exists(&code));
    assert!(
        f.state
            .reports
            .dir()
            .join(&code)
            .join("input.rvt")
            .is_file()
    );
}

#[tokio::test]
async fn a_fixed_marker_is_counted_before_a_new_report_pushes_it_out() {
    let mut f = setup();
    f.state.reports = Arc::new(Reports::with_limits(
        f._dir.path().join("reports"),
        20_000,
        chrono::Duration::days(7),
    ));
    seed_report(&f, "R-OLD000", None, 19_000);
    let marker = f.state.reports.dir().join("R-OLD000").join("FIXED.md");
    std::fs::write(marker, b"checked").unwrap();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let failed = wait_terminal(&f, &job.id).await;
    let code = failed.report_code.unwrap();
    assert_eq!(report_dirs(&f), [code]);
    assert_eq!(tally(&f), (1, 1, 1));
}

fn error_lines(f: &Fixture) -> Vec<serde_json::Value> {
    let path = f._dir.path().join("errors").join("errors.jsonl");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test]
async fn every_failure_leaves_a_line_that_outlives_its_report() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 7, 2026, 2024, "fail", 2026);
    let code = wait_terminal(&f, &job.id).await.report_code.unwrap();
    f.state.reports.remove(&code).unwrap();
    f.state.stats.forget_report(&code);

    let lines = error_lines(&f);
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    let mut keys: Vec<&str> = line
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "at",
            "engine_build",
            "engine_version",
            "exit_code",
            "file_kind",
            "kind",
            "message",
            "report",
            "target",
            "year"
        ]
    );
    assert_eq!(line["report"], code.as_str());
    assert_eq!(line["kind"], "failed");
    assert_eq!(line["message"], "The conversion failed");
    assert_eq!(line["exit_code"], 5);
    assert_eq!(line["engine_version"], "engine-stub 0.1.0");
    assert_eq!(line["file_kind"], "rvt");
    assert_eq!(line["year"], 2026);
    assert_eq!(line["target"], 2024);
    let text = line.to_string();
    assert!(!text.contains("House"), "{text}");
    assert!(!text.contains("10.0.0.7"), "{text}");
    assert!(!text.contains("in.rvt"), "{text}");
    assert!(!text.contains(&job.id.to_string()), "{text}");
}

#[tokio::test]
async fn a_failure_of_a_file_already_kept_leaves_a_line_too() {
    let f = setup();
    start(&f, 1);
    let first = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    let code = wait_terminal(&f, &first.id).await.report_code.unwrap();
    let again = queue_job(&f, 2, 2026, 2024, "fail", 2026);
    wait_terminal(&f, &again.id).await;
    let lines = error_lines(&f);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["report"], code.as_str());
}

#[tokio::test]
async fn a_failure_with_no_report_leaves_a_line_without_one() {
    let f = setup();
    std::fs::write(f.state.reports.dir(), b"").unwrap();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "fail", 2026);
    wait_terminal(&f, &job.id).await;
    let lines = error_lines(&f);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["report"], serde_json::Value::Null);
}

#[tokio::test]
async fn a_refused_file_leaves_no_line() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "refuse", 2026);
    wait_terminal(&f, &job.id).await;
    assert!(error_lines(&f).is_empty());
}

#[tokio::test]
async fn paths_with_spaces_are_cleaned_from_what_the_engine_said() {
    let dir = tempfile::Builder::new()
        .prefix("data dir ")
        .tempdir()
        .unwrap();
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
    ));
    let f = Fixture {
        state: common::test_state(dir.path(), clock),
        _dir: dir,
    };
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "path-error", 2026);
    wait_terminal(&f, &job.id).await;
    let lines = error_lines(&f);
    assert_eq!(
        lines[0]["message"],
        "Cannot read [path]: it is locked. Family in [file]. Also missing [path]"
    );
}

#[tokio::test]
async fn an_unreadable_file_is_not_converted_kept_or_counted() {
    let f = setup();
    start(&f, 1);
    let job = queue_job(&f, 1, 2026, 2024, "incomplete", 2026);
    let waiting = wait_terminal(&f, &job.id).await;
    assert_eq!(waiting.state, JobState::NeedsCheck);
    assert!(waiting.incomplete);
    assert!(waiting.error.is_none() && waiting.report_code.is_none());
    // No conversion ran: no output, no log, the input is still there.
    assert!(f.state.store.input_path(&job).is_file());
    assert!(!f.state.store.output_path(&job).exists());
    assert!(!f.state.store.log_path(&job).exists());
    assert!(report_dirs(&f).is_empty());
    assert_eq!(tally(&f), (0, 0, 0));

    // Confirmed: converted without the check, and the failure is kept and counted as usual.
    f.state.store.confirm(&job.id).unwrap();
    let failed = wait_terminal(&f, &job.id).await;
    assert_eq!(failed.state, JobState::Failed);
    assert_eq!(failed.error.unwrap().kind, ErrorKind::Corrupt);
    assert!(failed.report_owned && failed.confirmed_opens_in_revit);
    assert_eq!(report_dirs(&f).len(), 1);
    assert_eq!(tally(&f), (1, 1, 0));
}

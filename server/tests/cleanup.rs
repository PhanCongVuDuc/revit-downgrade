mod common;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode};
use revit_downgrade_server::cleanup::{sweep, wipe_jobs_dir};
use revit_downgrade_server::clock::ManualClock;
use revit_downgrade_server::errors::ErrorLine;
use revit_downgrade_server::jobs::{Job, NewJob};
use revit_downgrade_server::model::{ErrorKind, FileKind, JobState};
use revit_downgrade_server::{AppState, app};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

struct Fixture {
    state: AppState,
    clock: Arc<ManualClock>,
    app: Router,
    dir: tempfile::TempDir,
}

fn setup() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("reports")).unwrap();
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
    ));
    let state = common::test_state(dir.path(), clock.clone());
    let app = app(state.clone());
    Fixture {
        state,
        clock,
        app,
        dir,
    }
}

fn run_sweep(f: &Fixture) {
    sweep(
        &f.state.store,
        &f.state.reports,
        &f.state.stats,
        &f.state.errors,
        f.state.clock.now(),
    );
}

fn new_job(f: &Fixture, last: u8) -> Job {
    let req = NewJob {
        name: "Secret House.rvt".into(),
        size: 2048,
        kind: FileKind::Rvt,
        year: 2026,
        target: 2025,
    };
    f.state
        .store
        .create(
            req,
            std::net::IpAddr::from([10, 0, 0, last]),
            &f.state.pairs,
        )
        .unwrap()
}

/// A job that is queued with its input on disk.
fn queued_job(f: &Fixture, last: u8) -> Job {
    let job = new_job(f, last);
    std::fs::write(f.state.store.input_path(&job), b"input bytes").unwrap();
    f.state.store.mark_uploaded(&job.id);
    job
}

/// A job that ran and failed: input and log are on disk.
fn failed_job(f: &Fixture, last: u8) -> Job {
    let job = queued_job(f, last);
    let running = f.state.store.take_next().unwrap();
    assert_eq!(running.id, job.id);
    std::fs::write(f.state.store.log_path(&job), b"{\"type\":\"run\"}\n").unwrap();
    f.state.store.finish(
        &job.id,
        Err((ErrorKind::Failed, "The conversion failed.".into())),
    );
    f.state.store.get(&job.id).unwrap()
}

fn done_job(f: &Fixture, last: u8) -> Job {
    let job = queued_job(f, last);
    f.state.store.take_next().unwrap();
    std::fs::remove_file(f.state.store.input_path(&job)).unwrap();
    std::fs::write(f.state.store.output_path(&job), b"output bytes").unwrap();
    f.state.store.finish(&job.id, Ok(Vec::new()));
    f.state.store.get(&job.id).unwrap()
}

fn advance(f: &Fixture, secs: u64) {
    f.clock.advance(Duration::from_secs(secs));
}

async fn send(f: &Fixture, req: Request<Body>) -> axum::response::Response {
    let mut req = req;
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 4000))));
    f.app.clone().oneshot(req).await.unwrap()
}

async fn body_json(resp: axum::response::Response) -> Value {
    let b = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice(&b).unwrap()
}

fn post_open_report(id: &str, size: u64) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(format!("/api/jobs/{id}/open-report"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "consent": true, "size": size }).to_string(),
        ))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn report_dirs(f: &Fixture) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(f.dir.path().join("reports"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect()
}

#[test]
fn waiting_upload_removed_after_1h() {
    let f = setup();
    let job = new_job(&f, 1);
    advance(&f, 3600);
    run_sweep(&f);
    assert!(
        f.state.store.get(&job.id).is_some(),
        "exactly 1 h is not over"
    );
    advance(&f, 1);
    run_sweep(&f);
    assert!(f.state.store.get(&job.id).is_none());
    assert!(!f.state.store.job_dir(&job).exists());
}

#[tokio::test]
async fn done_output_removed_after_2h_then_410() {
    let f = setup();
    let job = done_job(&f, 1);
    let id = job.id.to_string();
    advance(&f, 7200);
    run_sweep(&f);
    assert_eq!(f.state.store.get(&job.id).unwrap().state, JobState::Done);
    advance(&f, 1);
    run_sweep(&f);
    assert_eq!(f.state.store.get(&job.id).unwrap().state, JobState::Expired);
    assert!(!f.state.store.job_dir(&job).exists());
    assert_eq!(
        send(&f, get(&format!("/api/jobs/{id}"))).await.status(),
        StatusCode::GONE
    );
    assert_eq!(
        send(&f, get(&format!("/api/jobs/{id}/file")))
            .await
            .status(),
        StatusCode::GONE
    );
    // The record stays for another 24 h, then goes.
    advance(&f, 24 * 3600);
    run_sweep(&f);
    assert!(f.state.store.get(&job.id).is_some());
    advance(&f, 1);
    run_sweep(&f);
    assert!(f.state.store.get(&job.id).is_none());
}

#[test]
fn failed_input_removed_after_15_min_without_consent() {
    let f = setup();
    let job = failed_job(&f, 1);
    advance(&f, 15 * 60);
    run_sweep(&f);
    assert!(f.state.store.input_path(&job).exists());
    advance(&f, 1);
    run_sweep(&f);
    assert_eq!(f.state.store.get(&job.id).unwrap().state, JobState::Expired);
    assert!(!f.state.store.job_dir(&job).exists());
}

#[test]
fn cancelled_stray_job_removed() {
    let f = setup();
    let job = new_job(&f, 1);
    f.state.store.cancel(&job.id);
    run_sweep(&f);
    assert!(f.state.store.get(&job.id).is_none());
    assert!(!f.state.store.job_dir(&job).exists());
}

#[test]
fn queued_and_running_untouched_by_sweep() {
    let f = setup();
    let running = queued_job(&f, 1);
    f.state.store.take_next().unwrap();
    let queued = queued_job(&f, 2);
    advance(&f, 30 * 24 * 3600);
    run_sweep(&f);
    assert_eq!(
        f.state.store.get(&running.id).unwrap().state,
        JobState::Running
    );
    assert_eq!(
        f.state.store.get(&queued.id).unwrap().state,
        JobState::Queued
    );
    assert!(f.state.store.input_path(&running).exists());
    assert!(f.state.store.input_path(&queued).exists());
}

#[test]
fn startup_wipes_jobs_dir() {
    let d = tempfile::tempdir().unwrap();
    let jobs = d.path().join("jobs");
    std::fs::create_dir_all(jobs.join("abc")).unwrap();
    std::fs::write(jobs.join("abc").join("in.rvt"), b"x").unwrap();
    std::fs::create_dir_all(d.path().join("reports").join("R-AAAAAA")).unwrap();
    wipe_jobs_dir(d.path()).unwrap();
    assert!(jobs.is_dir());
    assert_eq!(std::fs::read_dir(&jobs).unwrap().count(), 0);
    assert!(d.path().join("reports").join("R-AAAAAA").is_dir());
    // Also fine when nothing exists yet.
    let e = tempfile::tempdir().unwrap();
    wipe_jobs_dir(e.path()).unwrap();
    assert!(e.path().join("jobs").is_dir());
}

#[test]
fn cancelled_running_job_left_for_worker() {
    let f = setup();
    let job = queued_job(&f, 1);
    f.state.store.take_next().unwrap();
    f.state.store.cancel(&job.id);
    assert_eq!(
        f.state.store.get(&job.id).unwrap().state,
        JobState::Cancelled
    );
    advance(&f, 24 * 3600);
    run_sweep(&f);
    assert!(f.state.store.get(&job.id).is_some());
    assert!(f.state.store.input_path(&job).exists());
}

#[test]
fn orphan_dir_removed_on_next_sweep() {
    let f = setup();
    let jobs = f.dir.path().join("jobs");
    let unknown = jobs.join("0".repeat(32));
    std::fs::create_dir_all(&unknown).unwrap();
    std::fs::write(unknown.join("in.rvt"), b"x").unwrap();
    let junk = jobs.join("junk");
    std::fs::create_dir_all(&junk).unwrap();
    std::fs::write(jobs.join("stray.txt"), b"x").unwrap();
    run_sweep(&f);
    assert!(!unknown.exists());
    assert!(!junk.exists());
    assert!(!jobs.join("stray.txt").exists());
}

#[test]
fn expired_job_dir_retried() {
    let f = setup();
    let job = done_job(&f, 1);
    advance(&f, 7201);
    run_sweep(&f);
    assert_eq!(f.state.store.get(&job.id).unwrap().state, JobState::Expired);
    // Pretend the first delete failed: the directory is back.
    let dir = f.state.store.job_dir(&job);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("out.rvt"), b"left over").unwrap();
    run_sweep(&f);
    assert!(!dir.exists());
    assert!(f.state.store.get(&job.id).is_some(), "record kept for 410");
}

#[test]
fn live_job_dirs_untouched() {
    let f = setup();
    let failed = failed_job(&f, 3);
    let done = done_job(&f, 4);
    let waiting = new_job(&f, 1);
    let queued = queued_job(&f, 2);
    run_sweep(&f);
    for job in [&waiting, &queued, &failed, &done] {
        assert!(f.state.store.job_dir(job).is_dir(), "{:?}", job.state);
    }
}

#[test]
fn slow_upload_survives_while_data_arrives() {
    let f = setup();
    let job = new_job(&f, 1);
    for _ in 0..3 {
        advance(&f, 50 * 60);
        f.state.store.add_received(&job.id, 1);
        run_sweep(&f);
        assert!(f.state.store.get(&job.id).is_some());
    }
    advance(&f, 3601);
    run_sweep(&f);
    assert!(f.state.store.get(&job.id).is_none());
}

#[test]
fn sweep_prunes_old_client_history() {
    let f = setup();
    let job = new_job(&f, 1);
    f.state.store.remove(&job.id);
    assert_eq!(f.state.store.history_len(), 1);
    advance(&f, 3600);
    run_sweep(&f);
    assert_eq!(f.state.store.history_len(), 0);
}

#[tokio::test]
async fn expired_job_forgets_its_name() {
    let f = setup();
    let job = done_job(&f, 1);
    advance(&f, 7201);
    run_sweep(&f);
    let after = f.state.store.get(&job.id).unwrap();
    assert_eq!(after.state, JobState::Expired);
    assert_eq!(after.name, "");
}

#[tokio::test]
async fn done_job_downloads_and_reports_until_2_hours() {
    let f = setup();
    let job = done_job(&f, 1);
    let id = job.id.to_string();
    // 1 h 59 min after the conversion.
    advance(&f, 119 * 60);
    run_sweep(&f);
    assert_eq!(
        send(&f, get(&format!("/api/jobs/{id}/file")))
            .await
            .status(),
        StatusCode::OK
    );
    let resp = send(&f, post_open_report(&id, job.size)).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    // 2 h 1 min after the conversion.
    advance(&f, 2 * 60);
    run_sweep(&f);
    let resp = send(&f, post_open_report(&id, job.size)).await;
    assert_eq!(resp.status(), StatusCode::GONE);
    assert_eq!(body_json(resp).await["error"]["code"], "expired");
    assert!(report_dirs(&f).is_empty());
}

#[tokio::test]
async fn idle_open_report_closed_by_sweep() {
    let f = setup();
    let job = done_job(&f, 1);
    let id = job.id.to_string();
    let resp = send(&f, post_open_report(&id, job.size)).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let partial = f.state.store.report_input_path(&job);
    std::fs::write(&partial, b"part").unwrap();
    f.state.store.open_report_add_received(&job.id, 4);
    advance(&f, 3600);
    run_sweep(&f);
    assert!(f.state.store.get(&job.id).unwrap().open_report.is_some());
    advance(&f, 1);
    run_sweep(&f);
    let after = f.state.store.get(&job.id).unwrap();
    assert_eq!(after.state, JobState::Done);
    assert!(after.open_report.is_none());
    assert!(!partial.exists());
    assert!(f.state.store.output_path(&job).is_file());
}

// Reports and the numbers.

const WEEK: u64 = 7 * 24 * 3600;
const FILE: &str = "00112233445566778899aabbccddeeff";

/// `(failed, pending, fixed)`.
fn tally(f: &Fixture) -> (u64, u64, u64) {
    let s = f.state.stats.snapshot().expect("statistics available");
    (s.failed, s.pending, s.fixed)
}

/// A report directory made just now, written as the given age says.
fn report_on_disk(f: &Fixture, code: &str, age_s: i64) -> std::path::PathBuf {
    let dir = f.dir.path().join("reports").join(code);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("input.rvt"), b"input bytes").unwrap();
    let at = f.state.clock.now() - chrono::Duration::seconds(age_s);
    let meta = serde_json::json!({
        "engine_version": "e", "error_kind": "failed", "failed_at": at.to_rfc3339(), "kind": "rvt", "size": 11,
    });
    std::fs::write(dir.join("meta.json"), meta.to_string()).unwrap();
    dir
}

/// A report the statistics follow, for a file that fails.
fn tracked_report(f: &Fixture, code: &str, file: Option<&str>) -> std::path::PathBuf {
    let dir = report_on_disk(f, code, 0);
    let ip = std::net::IpAddr::from([10, 0, 0, 9]);
    f.state.stats.record_failure_kept(ip, file, 2024, code, &[]);
    dir
}

#[test]
fn a_fixed_marker_is_counted_once() {
    let f = setup();
    let dir = tracked_report(&f, "R-AAAAAA", Some(FILE));
    run_sweep(&f);
    assert_eq!(tally(&f), (1, 1, 0));
    std::fs::write(dir.join("FIXED.md"), b"checked in Revit 2024").unwrap();
    run_sweep(&f);
    assert_eq!(tally(&f), (0, 0, 1));
    run_sweep(&f);
    assert_eq!(tally(&f), (0, 0, 1));
    assert!(dir.is_dir(), "a fixed report stays until its time is up");
}

#[test]
fn a_fixed_marker_is_counted_before_its_report_expires() {
    let f = setup();
    let dir = tracked_report(&f, "R-AAAAAA", Some(FILE));
    std::fs::write(dir.join("FIXED.md"), b"checked").unwrap();
    advance(&f, WEEK + 1);
    run_sweep(&f);
    assert!(!dir.exists());
    assert_eq!(tally(&f), (0, 0, 1));
    run_sweep(&f);
    assert_eq!(tally(&f), (0, 0, 1));
}

#[test]
fn a_fixed_marker_on_a_report_not_tracked_counts_nothing() {
    let f = setup();
    let dir = report_on_disk(&f, "R-LEGACY", 3600);
    std::fs::write(dir.join("FIXED.md"), b"checked").unwrap();
    run_sweep(&f);
    assert_eq!(tally(&f), (0, 0, 0));
    assert!(dir.is_dir());
}

#[test]
fn a_report_gone_by_hand_or_by_age_lowers_pending_only() {
    let f = setup();
    let by_hand = tracked_report(&f, "R-AAAAAA", Some(FILE));
    tracked_report(&f, "R-BBBBBB", None);
    assert_eq!(tally(&f), (2, 2, 0));
    std::fs::remove_dir_all(by_hand).unwrap();
    run_sweep(&f);
    assert_eq!(tally(&f), (2, 1, 0));
    advance(&f, WEEK + 1);
    run_sweep(&f);
    assert_eq!(tally(&f), (2, 0, 0));
    assert!(report_dirs(&f).is_empty());
}

#[test]
fn a_counted_report_that_goes_changes_no_number() {
    let f = setup();
    let dir = tracked_report(&f, "R-AAAAAA", Some(FILE));
    std::fs::write(dir.join("FIXED.md"), b"checked").unwrap();
    run_sweep(&f);
    std::fs::remove_dir_all(&dir).unwrap();
    run_sweep(&f);
    assert_eq!(tally(&f), (0, 0, 1));
    assert!(f.state.stats.tracked_reports().is_empty());
}

#[test]
fn a_report_from_before_this_release_goes_after_7_days() {
    let f = setup();
    let stale = report_on_disk(&f, "R-OLDONE", WEEK as i64 + 1);
    let fresh = report_on_disk(&f, "R-NEWONE", WEEK as i64 - 60);
    run_sweep(&f);
    assert!(!stale.exists());
    assert!(fresh.is_dir());
    assert_eq!(tally(&f), (0, 0, 0));
}

#[test]
fn a_sweep_prunes_the_error_log() {
    let f = setup();
    let line = ErrorLine {
        at: f.state.clock.now() - chrono::Duration::days(91),
        report: None,
        kind: "failed".into(),
        message: None,
        exit_code: None,
        engine_version: "e".into(),
        engine_build: "b".into(),
        file_kind: "rvt".into(),
        year: 2026,
        target: 2024,
        confirmed_opens_in_revit: false,
    };
    f.state.errors.append(&line);
    let log = f.dir.path().join("errors").join("errors.jsonl");
    assert!(!std::fs::read(&log).unwrap().is_empty());
    run_sweep(&f);
    assert!(std::fs::read(&log).unwrap().is_empty());
}

#[tokio::test]
async fn a_job_left_waiting_for_a_check_is_removed_after_1h_and_counts_nothing() {
    let f = setup();
    let job = queued_job(&f, 1);
    f.state.store.take_next().unwrap();
    assert!(f.state.store.finish_needs_check(&job.id, true));
    let before = f.state.stats.snapshot().unwrap();
    advance(&f, 3600);
    run_sweep(&f);
    assert_eq!(f.state.store.get(&job.id).unwrap().state, JobState::NeedsCheck);
    assert!(f.state.store.input_path(&job).is_file());
    advance(&f, 1);
    run_sweep(&f);
    assert_eq!(f.state.store.get(&job.id).unwrap().state, JobState::Expired);
    assert!(!f.state.store.job_dir(&job).exists());
    let resp = send(&f, get(&format!("/api/jobs/{}", job.id))).await;
    assert_eq!(resp.status(), StatusCode::GONE);
    let after = f.state.stats.snapshot().unwrap();
    assert_eq!(
        (after.people, after.succeeded, after.failed, after.pending),
        (before.people, before.succeeded, before.failed, before.pending)
    );
    assert!(report_dirs(&f).is_empty());
}

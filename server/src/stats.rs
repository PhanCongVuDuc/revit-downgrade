//! Usage statistics: running totals kept in `<data>/stats/` across restarts.
//!
//! `stats.json` holds the start date, the succeeded, failed and reported conversion counts, one
//! keyed hash per client network address, and one keyed hash per file that still fails. `key`
//! holds the secret for those hashes. No address, file name or file content is ever stored here.
//!
//! `failed` counts files, not attempts: sending a failing file again changes nothing, and the
//! file leaves the count once it converts. A success later reported as not opening in Revit moves
//! to the failures, and only a newer engine can take such a file out again, because the same
//! engine would report the same success.

use crate::jobs::rate_key;
use chrono::{DateTime, NaiveDate, Utc};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

const VERSION: u32 = 1;
const STATS_FILE: &str = "stats.json";
const KEY_FILE: &str = "key";
const KEY_BYTES: usize = 32;
/// Bytes of the keyed hash kept per client: 128 bits, rendered as 32 hex characters.
const PERSON_BYTES: usize = 16;

/// A file that still fails. What a failure proves goes one way: a file failing on the way to
/// `target` also fails on the way to anything older, so only a success at `target` or older
/// clears it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FailedFile {
    /// The newest target it failed for.
    target: u16,
    /// The engine build it last failed on.
    engine: String,
    /// It converted, but was reported as not opening in Revit.
    #[serde(default)]
    unopenable: bool,
    /// The report holding this file, sent while `engine` was running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    report: Option<String>,
}

/// The public totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub since: String,
    pub people: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub contributed: u64,
}

/// `stats.json` as read from disk. Every field is required except `failed_files`, which older
/// files do not have; unknown fields are ignored.
#[derive(Deserialize)]
struct StatsFile {
    version: u32,
    since: String,
    succeeded: u64,
    failed: u64,
    contributed: u64,
    people: Vec<String>,
    #[serde(default)]
    failed_files: HashMap<String, FailedFile>,
}

/// `stats.json` as written: borrows the in-memory totals instead of copying the people set.
#[derive(Serialize)]
struct StatsFileOut<'a> {
    version: u32,
    since: &'a str,
    succeeded: u64,
    failed: u64,
    contributed: u64,
    people: &'a HashSet<String>,
    failed_files: &'a HashMap<String, FailedFile>,
}

struct Totals {
    since: String,
    succeeded: u64,
    failed: u64,
    contributed: u64,
    people: HashSet<String>,
    failed_files: HashMap<String, FailedFile>,
}

struct Available {
    totals: Totals,
    key: [u8; KEY_BYTES],
    /// The last write failed. Every change writes all totals anyway; this only decides whether
    /// a successful write is logged as a recovery.
    dirty: bool,
}

/// Running totals, or nothing when the files on disk could not be trusted. There is no way
/// back from `Unavailable` while running: the operator fixes or removes the files and restarts.
enum State {
    Available(Box<Available>),
    Unavailable,
}

/// The usage statistics of one data directory. Every change is written to disk before the
/// call returns; a failed write is logged and never reaches the caller.
pub struct Stats {
    dir: PathBuf,
    state: Mutex<State>,
    /// What `/api/stats` serves. Replaced after each write attempt, under its own lock, so that
    /// readers never wait for a write to the disk.
    published: Mutex<Option<Snapshot>>,
    /// Identifies the running engine build, so that a new build gets a new chance at old files.
    engine: String,
}

impl Stats {
    /// Opens (or starts) the statistics under `dir`. Never fails: files that cannot be read or
    /// are not valid are left untouched and the statistics become unavailable.
    pub fn open(dir: PathBuf, now: DateTime<Utc>) -> Stats {
        let state = match load(&dir, now) {
            Ok(a) => State::Available(Box::new(a)),
            Err(reason) => {
                tracing::error!(
                    dir = %dir.display(),
                    "usage statistics unavailable until the files are fixed or removed and the server restarts: {reason}"
                );
                State::Unavailable
            }
        };
        let published = Mutex::new(snapshot_of(&state));
        Stats {
            dir,
            state: Mutex::new(state),
            published,
            engine: String::new(),
        }
    }

    /// Sets what identifies the running engine build.
    pub fn with_engine_build(mut self, engine: String) -> Stats {
        self.engine = engine;
        self
    }

    /// The stored identity of a file: a keyed hash of its content, or `None` while the statistics
    /// are unavailable. Reads the whole file, so call it off the async threads.
    pub fn file_id(&self, path: &Path) -> io::Result<Option<String>> {
        let key = match &*self.lock() {
            State::Available(a) => a.key,
            State::Unavailable => return Ok(None),
        };
        file_id(&key, path).map(Some)
    }

    /// Counts a finished conversion and the client it came from. A file that was failing leaves
    /// the failures, if this success proves it: see `FailedFile`.
    pub fn record_success(&self, ip: IpAddr, file: Option<&str>, target: u16) {
        let engine = &self.engine;
        self.change(|a| {
            a.totals.succeeded += 1;
            a.totals.people.insert(person_id(&a.key, ip));
            let Some(file) = file else { return };
            let cleared = a
                .totals
                .failed_files
                .get(file)
                .is_some_and(|f| target <= f.target && (!f.unopenable || f.engine != *engine));
            if cleared {
                a.totals.failed_files.remove(file);
                a.totals.failed = a.totals.failed.saturating_sub(1);
            }
        });
    }

    /// The report that already holds this file, if it was sent while this engine build was
    /// running: sending it again would add nothing. After a release a file that still fails can
    /// be sent again.
    pub fn known_report(&self, file: &str) -> Option<String> {
        match &*self.lock() {
            State::Available(a) => a
                .totals
                .failed_files
                .get(file)
                .filter(|f| f.engine == self.engine)
                .and_then(|f| f.report.clone()),
            State::Unavailable => None,
        }
    }

    /// Counts a failed conversion and the client it came from. A file already counted as failing
    /// is not counted again. A newer engine build forgets the file's report.
    pub fn record_failure(&self, ip: IpAddr, file: Option<&str>, target: u16) {
        let engine = &self.engine;
        self.change(|a| {
            a.totals.people.insert(person_id(&a.key, ip));
            let Some(file) = file else {
                a.totals.failed += 1;
                return;
            };
            match a.totals.failed_files.get_mut(file) {
                Some(f) => {
                    f.target = f.target.max(target);
                    if f.engine != *engine {
                        f.engine = engine.clone();
                        f.report = None;
                        // A new engine has not been shown to make a file that does not open.
                        f.unopenable = false;
                    }
                }
                None => {
                    a.totals.failed += 1;
                    a.totals.failed_files.insert(
                        file.to_string(),
                        FailedFile {
                            target,
                            engine: engine.clone(),
                            unopenable: false,
                            report: None,
                        },
                    );
                }
            }
        });
    }

    /// Counts the client of a conversion that did not fail through the engine's fault, such as a
    /// target this file cannot go to.
    pub fn record_rejected(&self, ip: IpAddr) {
        self.change(|a| {
            a.totals.people.insert(person_id(&a.key, ip));
        });
    }

    /// Counts one saved failure report and remembers it for the file.
    pub fn record_contribution(&self, file: Option<&str>, code: &str) {
        self.change(|a| {
            a.totals.contributed += 1;
            if let Some(f) = file.and_then(|file| a.totals.failed_files.get_mut(file)) {
                f.report = Some(code.to_string());
            }
        });
    }

    /// Counts a converted file the user reported as not opening in Revit: one success becomes a
    /// failure (never below zero, and not a second failure for the same file) and one report is
    /// added, in a single write. The client was already counted when the job succeeded.
    pub fn record_unopenable(&self, file: Option<&str>, target: u16, code: &str) {
        let engine = &self.engine;
        self.change(|a| {
            a.totals.succeeded = a.totals.succeeded.saturating_sub(1);
            a.totals.contributed += 1;
            let Some(file) = file else {
                a.totals.failed += 1;
                return;
            };
            let entry = FailedFile {
                target,
                engine: engine.clone(),
                unopenable: true,
                report: Some(code.to_string()),
            };
            match a.totals.failed_files.get_mut(file) {
                Some(f) => {
                    let newest = f.target.max(target);
                    *f = entry;
                    f.target = newest;
                }
                None => {
                    a.totals.failed += 1;
                    a.totals.failed_files.insert(file.to_string(), entry);
                }
            }
        });
    }

    /// The current totals, or `None` while the statistics are unavailable.
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.published
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Applies a change and writes the result, both under the lock so that concurrent changes
    /// are written in order and none is lost.
    fn change(&self, f: impl FnOnce(&mut Available)) {
        let mut g = self.lock();
        let State::Available(a) = &mut *g else { return };
        f(a);
        match write_totals(&self.dir, &a.totals) {
            Ok(()) => {
                if a.dirty {
                    tracing::info!("usage statistics written again");
                }
                a.dirty = false;
            }
            Err(e) => {
                tracing::warn!(error = ?e.kind(), "could not write usage statistics; kept in memory");
                a.dirty = true;
            }
        }
        // Published after the write, still under the state lock so publications stay in order.
        *self.published.lock().unwrap_or_else(|e| e.into_inner()) = snapshot_of(&g);
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn snapshot_of(state: &State) -> Option<Snapshot> {
    match state {
        State::Available(a) => Some(Snapshot {
            since: a.totals.since.clone(),
            people: a.totals.people.len() as u64,
            succeeded: a.totals.succeeded,
            failed: a.totals.failed,
            contributed: a.totals.contributed,
        }),
        State::Unavailable => None,
    }
}

/// Reads (or creates) the key and the totals. An error is the reason the statistics are
/// unavailable; nothing on disk has been changed by then except removing leftover temp files
/// and creating a missing key.
fn load(dir: &Path, now: DateTime<Utc>) -> Result<Available, String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create the directory: {e}"))?;
    for name in [STATS_FILE, KEY_FILE] {
        let tmp = tmp_path(&dir.join(name));
        if let Err(e) = fs::remove_file(&tmp)
            && e.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(error = ?e.kind(), "could not remove a leftover statistics temp file");
        }
    }

    let stats_path = dir.join(STATS_FILE);
    let key = load_key(&dir.join(KEY_FILE), &stats_path)?;

    let totals = match fs::read(&stats_path) {
        Ok(bytes) => parse_totals(&bytes).map_err(|e| format!("{STATS_FILE} is not valid: {e}"))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let fresh = Totals {
                since: now.format("%Y-%m-%d").to_string(),
                succeeded: 0,
                failed: 0,
                contributed: 0,
                people: HashSet::new(),
                failed_files: HashMap::new(),
            };
            let dirty = match write_totals(dir, &fresh) {
                Ok(()) => false,
                Err(e) => {
                    tracing::warn!(error = ?e.kind(), "could not write new usage statistics");
                    true
                }
            };
            return Ok(Available {
                totals: fresh,
                key,
                dirty,
            });
        }
        Err(e) => return Err(format!("cannot read {STATS_FILE}: {e}")),
    };
    Ok(Available {
        totals,
        key,
        dirty: false,
    })
}

/// Reads the key, or makes one when the file does not exist. A key file that is there but
/// cannot be read or is not valid is never replaced: that would count everyone again.
fn load_key(path: &Path, stats_path: &Path) -> Result<[u8; KEY_BYTES], String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let hex = text.strip_suffix('\n').unwrap_or(&text);
            parse_hex::<KEY_BYTES>(hex).ok_or_else(|| {
                format!(
                    "{KEY_FILE} must hold {} lowercase hex characters",
                    KEY_BYTES * 2
                )
            })
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if stats_path.exists() {
                tracing::warn!(
                    "statistics key missing: made a new one; clients counted before may be counted again"
                );
            }
            let mut key = [0u8; KEY_BYTES];
            getrandom::fill(&mut key).map_err(|e| format!("no random source for the key: {e}"))?;
            write_atomic(path, to_hex(&key).as_bytes())
                .map_err(|e| format!("cannot write {KEY_FILE}: {e}"))?;
            Ok(key)
        }
        Err(e) => Err(format!("cannot read {KEY_FILE}: {e}")),
    }
}

fn parse_totals(bytes: &[u8]) -> Result<Totals, String> {
    let file: StatsFile = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if file.version != VERSION {
        return Err(format!("unknown version {}", file.version));
    }
    if NaiveDate::parse_from_str(&file.since, "%Y-%m-%d").is_err() {
        return Err("since is not a YYYY-MM-DD date".into());
    }
    if let Some(bad) = file
        .people
        .iter()
        .position(|p| parse_hex::<PERSON_BYTES>(p).is_none())
    {
        return Err(format!(
            "people[{bad}] is not {} lowercase hex characters",
            PERSON_BYTES * 2
        ));
    }
    if file
        .failed_files
        .keys()
        .any(|f| parse_hex::<PERSON_BYTES>(f).is_none())
    {
        return Err(format!(
            "a key of failed_files is not {} lowercase hex characters",
            PERSON_BYTES * 2
        ));
    }
    Ok(Totals {
        since: file.since,
        succeeded: file.succeeded,
        failed: file.failed,
        contributed: file.contributed,
        people: file.people.into_iter().collect(),
        failed_files: file.failed_files,
    })
}

fn write_totals(dir: &Path, t: &Totals) -> io::Result<()> {
    let out = StatsFileOut {
        version: VERSION,
        since: &t.since,
        succeeded: t.succeeded,
        failed: t.failed,
        contributed: t.contributed,
        people: &t.people,
        failed_files: &t.failed_files,
    };
    let bytes = serde_json::to_vec(&out).map_err(io::Error::other)?;
    write_atomic(&dir.join(STATS_FILE), &bytes)
}

/// Writes `<path>.tmp`, flushes it to disk and renames it over `path`, so the file on disk is
/// always either the old or the new content.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = tmp_path(path);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    // Make the rename itself durable across a power loss. The file is already replaced as a
    // whole by now, so a file system that cannot sync a directory does not fail the write.
    #[cfg(unix)]
    if let Some(parent) = path.parent()
        && let Err(e) = fs::File::open(parent).and_then(|d| d.sync_all())
    {
        tracing::debug!(error = ?e.kind(), "could not sync the statistics directory");
    }
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// The stored identity of a client: the first 16 bytes of HMAC-SHA256 over its rate-limit
/// address (IPv6 grouped by /64), as hex.
fn person_id(key: &[u8; KEY_BYTES], ip: IpAddr) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes keys of any length");
    mac.update(rate_key(ip).to_string().as_bytes());
    let digest = mac.finalize().into_bytes();
    to_hex(&digest[..PERSON_BYTES])
}

/// What identifies an engine build: the hash of its program. The engine's version number does not
/// change between releases, its program does. Never fails: an engine that cannot be read counts
/// as one unknown build, so reports are simply not offered again until it can.
pub fn engine_build(path: &Path) -> String {
    file_id(&[0; KEY_BYTES], path).unwrap_or_else(|e| {
        tracing::warn!(error = ?e.kind(), "could not read the engine program");
        "unknown".into()
    })
}

/// The stored identity of a file: the first 16 bytes of HMAC-SHA256 over its content, as hex.
fn file_id(key: &[u8; KEY_BYTES], path: &Path) -> io::Result<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes keys of any length");
    let mut file = fs::File::open(path)?;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        match file.read(&mut buffer)? {
            0 => break,
            n => mac.update(&buffer[..n]),
        }
    }
    Ok(to_hex(&mac.finalize().into_bytes()[..PERSON_BYTES]))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Exactly `N * 2` lowercase hex characters.
fn parse_hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    let bytes = s.as_bytes();
    if bytes.len() != N * 2 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = digit(bytes[2 * i])? << 4 | digit(bytes[2 * i + 1])?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::path::Path;
    use std::sync::Arc;

    fn day(d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, d, 12, 0, 0).unwrap()
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn open(dir: &Path) -> Stats {
        Stats::open(dir.join("stats"), day(29))
    }

    fn stats_file(dir: &Path) -> PathBuf {
        dir.join("stats").join("stats.json")
    }

    fn key_file(dir: &Path) -> PathBuf {
        dir.join("stats").join("key")
    }

    fn counts(s: &Stats) -> (u64, u64, u64, u64) {
        let snap = s.snapshot().expect("statistics available");
        (snap.people, snap.succeeded, snap.failed, snap.contributed)
    }

    const VALID: &str = r#"{"version":1,"since":"2026-10-01","succeeded":4,"failed":2,"contributed":1,"people":["00112233445566778899aabbccddeeff"]}"#;

    /// Writes `stats.json` (and a valid key) before the first open.
    fn seed(dir: &Path, json: &str) {
        std::fs::create_dir_all(dir.join("stats")).unwrap();
        std::fs::write(stats_file(dir), json).unwrap();
        std::fs::write(key_file(dir), "ab".repeat(32)).unwrap();
    }

    #[test]
    fn first_open_starts_at_zero_today() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        assert_eq!(
            s.snapshot(),
            Some(Snapshot {
                since: "2026-10-29".into(),
                people: 0,
                succeeded: 0,
                failed: 0,
                contributed: 0,
            })
        );
        assert!(stats_file(dir.path()).is_file());
        assert!(key_file(dir.path()).is_file());
    }

    #[test]
    fn totals_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = open(dir.path());
            s.record_success(ip("203.0.113.7"), None, 2024);
            s.record_success(ip("203.0.113.7"), None, 2024);
            s.record_failure(ip("203.0.113.7"), None, 2024);
            s.record_contribution(None, "R-AAAAAA");
        }
        let s = Stats::open(dir.path().join("stats"), day(30));
        let snap = s.snapshot().unwrap();
        assert_eq!(snap.since, "2026-10-29");
        assert_eq!(counts(&s), (1, 2, 1, 1));
    }

    #[test]
    fn people_counted_by_address_and_ipv6_network() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        s.record_success(ip("203.0.113.7"), None, 2024);
        s.record_failure(ip("203.0.113.7"), None, 2024);
        assert_eq!(counts(&s).0, 1);
        s.record_success(ip("2001:db8:1:2::5"), None, 2024);
        s.record_success(ip("2001:db8:1:2:aaaa::9"), None, 2024);
        assert_eq!(counts(&s).0, 2);
        s.record_success(ip("198.51.100.4"), None, 2024);
        assert_eq!(counts(&s), (3, 4, 1, 0));
    }

    #[test]
    fn file_holds_no_address() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        s.record_success(ip("203.0.113.7"), None, 2024);
        s.record_failure(ip("2001:db8:1:2::5"), None, 2024);
        let text = std::fs::read_to_string(stats_file(dir.path())).unwrap();
        for needle in ["203.0.113.7", "2001:db8:1:2", "2001:db8"] {
            assert!(!text.contains(needle), "found {needle} in {text}");
        }
    }

    #[test]
    fn bad_stats_file_is_unavailable_and_kept() {
        let upper = VALID.replace(
            "00112233445566778899aabbccddeeff",
            "00112233445566778899AABBCCDDEEFF",
        );
        let short = VALID.replace("00112233445566778899aabbccddeeff", "0011");
        let cases = [
            "{".to_string(),
            VALID.replace(r#""version":1"#, r#""version":2"#),
            VALID.replace(r#","contributed":1"#, ""),
            VALID.replace(r#""failed":2"#, r#""failed":"2""#),
            upper,
            short,
        ];
        for bad in cases {
            let dir = tempfile::tempdir().unwrap();
            seed(dir.path(), &bad);
            let s = open(dir.path());
            assert_eq!(s.snapshot(), None, "{bad}");
            s.record_success(ip("203.0.113.7"), None, 2024);
            s.record_contribution(None, "R-AAAAAA");
            assert_eq!(s.snapshot(), None);
            assert_eq!(
                std::fs::read(stats_file(dir.path())).unwrap(),
                bad.as_bytes()
            );
        }
    }

    #[test]
    fn valid_seed_is_read() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), VALID);
        let s = open(dir.path());
        assert_eq!(s.snapshot().unwrap().since, "2026-10-01");
        assert_eq!(counts(&s), (1, 4, 2, 1));
    }

    #[test]
    fn bad_key_is_unavailable_and_kept() {
        for bad in [
            "nothex",
            &"AB".repeat(32),
            &"ab".repeat(31),
            &format!("{}\n\n", "ab".repeat(32)),
        ] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join("stats")).unwrap();
            std::fs::write(key_file(dir.path()), bad).unwrap();
            let s = open(dir.path());
            assert_eq!(s.snapshot(), None, "{bad:?}");
            s.record_success(ip("203.0.113.7"), None, 2024);
            assert_eq!(std::fs::read(key_file(dir.path())).unwrap(), bad.as_bytes());
            assert!(!stats_file(dir.path()).exists());
        }
    }

    #[test]
    fn key_with_trailing_newline_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("stats")).unwrap();
        std::fs::write(key_file(dir.path()), format!("{}\n", "ab".repeat(32))).unwrap();
        assert!(open(dir.path()).snapshot().is_some());
    }

    #[test]
    fn missing_key_is_regenerated_and_totals_kept() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = open(dir.path());
            s.record_success(ip("203.0.113.7"), None, 2024);
        }
        std::fs::remove_file(key_file(dir.path())).unwrap();
        let s = open(dir.path());
        assert_eq!(counts(&s), (1, 1, 0, 0));
        let key = std::fs::read_to_string(key_file(dir.path())).unwrap();
        assert_eq!(key.trim_end().len(), 64);
    }

    #[test]
    fn leftover_temp_files_are_ignored_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), VALID);
        let tmp = dir.path().join("stats").join("stats.json.tmp");
        std::fs::write(&tmp, r#"{"version":1,"since":"2026-10-01","succeeded":99"#).unwrap();
        let s = open(dir.path());
        assert_eq!(counts(&s), (1, 4, 2, 1));
        assert!(!tmp.exists());
    }

    #[test]
    fn failed_write_keeps_counts_for_the_next_write() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        // A directory where the temporary file should go makes every write fail.
        let blocker = dir.path().join("stats").join("stats.json.tmp");
        std::fs::create_dir(&blocker).unwrap();
        s.record_success(ip("203.0.113.7"), None, 2024);
        s.record_contribution(None, "R-AAAAAA");
        assert_eq!(counts(&s), (1, 1, 0, 1));
        let on_disk = std::fs::read_to_string(stats_file(dir.path())).unwrap();
        assert!(on_disk.contains(r#""succeeded":0"#), "{on_disk}");

        std::fs::remove_dir(&blocker).unwrap();
        s.record_failure(ip("198.51.100.4"), None, 2024);
        drop(s);
        let s = open(dir.path());
        assert_eq!(counts(&s), (2, 1, 1, 1));
    }

    #[test]
    fn concurrent_outcomes_are_all_counted() {
        let dir = tempfile::tempdir().unwrap();
        let s = Arc::new(open(dir.path()));
        let threads: Vec<_> = ["203.0.113.1", "203.0.113.2", "203.0.113.3"]
            .into_iter()
            .enumerate()
            .map(|(i, addr)| {
                let s = s.clone();
                std::thread::spawn(move || {
                    if i == 0 {
                        s.record_failure(ip(addr), None, 2024);
                    } else {
                        s.record_success(ip(addr), None, 2024);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(counts(&s), (3, 2, 1, 0));
        drop(s);
        assert_eq!(counts(&open(dir.path())), (3, 2, 1, 0));
    }

    #[test]
    fn unopenable_moves_one_success_to_failed_and_contributed() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = open(dir.path());
            for _ in 0..3 {
                s.record_success(ip("203.0.113.7"), None, 2024);
            }
            s.record_failure(ip("203.0.113.7"), None, 2024);
            assert_eq!(counts(&s), (1, 3, 1, 0));
            s.record_unopenable(None, 2024, "R-AAAAAA");
            assert_eq!(counts(&s), (1, 2, 2, 1));
        }
        assert_eq!(counts(&open(dir.path())), (1, 2, 2, 1));
    }

    #[test]
    fn unopenable_never_takes_succeeded_below_zero() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        s.record_unopenable(None, 2024, "R-AAAAAA");
        assert_eq!(counts(&s), (0, 0, 1, 1));
    }

    #[test]
    fn unopenable_is_ignored_while_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "{");
        let s = open(dir.path());
        s.record_unopenable(None, 2024, "R-AAAAAA");
        assert_eq!(s.snapshot(), None);
        assert_eq!(std::fs::read(stats_file(dir.path())).unwrap(), b"{");
    }

    #[test]
    fn snapshot_does_not_wait_for_a_write() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        s.record_success(ip("203.0.113.7"), None, 2024);
        // A change holds the state lock while it writes; readers must still get an answer.
        let _writing = s.lock();
        assert_eq!(s.snapshot().map(|snap| snap.succeeded), Some(1));
    }

    const FILE: &str = "00112233445566778899aabbccddeeff";
    const CLIENT: &str = "203.0.113.7";

    fn open_on(dir: &Path, engine: &str) -> Stats {
        open(dir).with_engine_build(engine.into())
    }

    #[test]
    fn a_failing_file_counts_once_however_often_it_is_sent() {
        let dir = tempfile::tempdir().unwrap();
        let s = open_on(dir.path(), "a");
        for _ in 0..3 {
            s.record_failure(ip(CLIENT), Some(FILE), 2024);
        }
        s.record_failure(ip("198.51.100.4"), Some(FILE), 2024);
        assert_eq!(counts(&s), (2, 0, 1, 0));
    }

    #[test]
    fn different_files_count_separately_and_unknown_files_always_count() {
        let dir = tempfile::tempdir().unwrap();
        let s = open_on(dir.path(), "a");
        s.record_failure(ip(CLIENT), Some(FILE), 2024);
        s.record_failure(ip(CLIENT), Some("ffeeddccbbaa99887766554433221100"), 2024);
        s.record_failure(ip(CLIENT), None, 2024);
        s.record_failure(ip(CLIENT), None, 2024);
        assert_eq!(counts(&s).2, 4);
    }

    #[test]
    fn a_file_that_converts_leaves_the_failures() {
        let dir = tempfile::tempdir().unwrap();
        open_on(dir.path(), "a").record_failure(ip(CLIENT), Some(FILE), 2024);
        let s = open_on(dir.path(), "b");
        s.record_success(ip(CLIENT), Some(FILE), 2024);
        assert_eq!(counts(&s), (1, 1, 0, 0));
        // It is forgotten: failing again is a new failure.
        s.record_failure(ip(CLIENT), Some(FILE), 2024);
        assert_eq!(counts(&s).2, 1);
    }

    #[test]
    fn a_shallower_success_proves_nothing_about_a_deeper_failure() {
        let dir = tempfile::tempdir().unwrap();
        let s = open_on(dir.path(), "a");
        s.record_failure(ip(CLIENT), Some(FILE), 2023);
        s.record_success(ip(CLIENT), Some(FILE), 2025);
        assert_eq!(counts(&s).2, 1);
        s.record_success(ip(CLIENT), Some(FILE), 2023);
        assert_eq!(counts(&s).2, 0);
    }

    #[test]
    fn a_known_report_is_named_only_on_the_same_engine() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = open_on(dir.path(), "a");
            s.record_failure(ip(CLIENT), Some(FILE), 2024);
            s.record_contribution(Some(FILE), "R-AAAAAA");
            assert_eq!(s.known_report(FILE).as_deref(), Some("R-AAAAAA"));
        }
        // After a release the same file may be sent again, and is still one failure.
        let s = open_on(dir.path(), "b");
        assert_eq!(s.known_report(FILE), None);
        s.record_failure(ip(CLIENT), Some(FILE), 2024);
        assert_eq!(s.known_report(FILE), None);
        assert_eq!(counts(&s).2, 1);
    }

    #[test]
    fn an_unopenable_file_counts_once_and_only_a_new_engine_clears_it() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = open_on(dir.path(), "a");
            for _ in 0..2 {
                s.record_success(ip(CLIENT), Some(FILE), 2024);
                s.record_unopenable(Some(FILE), 2024, "R-AAAAAA");
            }
            assert_eq!(counts(&s), (1, 0, 1, 2));
            // The same engine says success again, which proves nothing.
            s.record_success(ip(CLIENT), Some(FILE), 2024);
            assert_eq!(counts(&s), (1, 1, 1, 2));
        }
        let s = open_on(dir.path(), "b");
        s.record_success(ip(CLIENT), Some(FILE), 2024);
        assert_eq!(counts(&s), (1, 2, 0, 2));
    }

    #[test]
    fn a_failure_in_between_does_not_clear_an_unopenable_file() {
        let dir = tempfile::tempdir().unwrap();
        let s = open_on(dir.path(), "a");
        s.record_success(ip(CLIENT), Some(FILE), 2024);
        s.record_unopenable(Some(FILE), 2024, "R-AAAAAA");
        s.record_failure(ip(CLIENT), Some(FILE), 2022);
        s.record_success(ip(CLIENT), Some(FILE), 2024);
        assert_eq!(counts(&s).2, 1);
    }

    #[test]
    fn rejected_counts_the_client_only() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        s.record_rejected(ip(CLIENT));
        assert_eq!(counts(&s), (1, 0, 0, 0));
    }

    #[test]
    fn file_id_depends_on_content_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        let (a, b, c) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("c"),
        );
        std::fs::write(&a, b"same").unwrap();
        std::fs::write(&b, b"same").unwrap();
        std::fs::write(&c, b"other").unwrap();
        let id = s.file_id(&a).unwrap().unwrap();
        assert_eq!(id.len(), 32);
        assert_eq!(Some(id.clone()), s.file_id(&b).unwrap());
        assert_ne!(Some(id.clone()), s.file_id(&c).unwrap());
        let other = tempfile::tempdir().unwrap();
        let s2 = open(other.path());
        assert_ne!(Some(id), s2.file_id(&a).unwrap());
    }

    #[test]
    fn failed_files_survive_a_reopen_and_older_files_still_load() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), VALID);
        {
            let s = open_on(dir.path(), "a");
            assert_eq!(counts(&s), (1, 4, 2, 1));
            s.record_failure(ip(CLIENT), Some(FILE), 2024);
            assert_eq!(counts(&s).2, 3);
        }
        let s = open_on(dir.path(), "a");
        s.record_failure(ip(CLIENT), Some(FILE), 2024);
        assert_eq!(counts(&s).2, 3);
    }

    #[test]
    fn a_bad_failed_files_key_makes_the_statistics_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let bad = VALID.replace(
            r#""people":"#,
            r#""failed_files":{"xyz":{"target":2024,"engine":"a"}},"people":"#,
        );
        seed(dir.path(), &bad);
        assert_eq!(open(dir.path()).snapshot(), None);
    }

    /// Scale check: `cargo test -p revit-downgrade-server --release -- --ignored stats_perf`.
    #[test]
    #[ignore = "slow; run in release"]
    fn stats_perf() {
        const PEOPLE: usize = 500_000;
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path());
        {
            let mut g = s.lock();
            let State::Available(a) = &mut *g else {
                panic!("statistics unavailable")
            };
            let key = a.key;
            for i in 0..PEOPLE as u32 {
                a.totals
                    .people
                    .insert(person_id(&key, IpAddr::from(i.to_be_bytes())));
            }
        }

        let started = std::time::Instant::now();
        s.record_contribution(None, "R-AAAAAA");
        let write = started.elapsed();
        drop(s);

        let started = std::time::Instant::now();
        let s = open(dir.path());
        let read = started.elapsed();
        let size = std::fs::metadata(stats_file(dir.path())).unwrap().len();
        println!("{PEOPLE} people, {size} bytes: write {write:?}, open {read:?}");
        assert_eq!(counts(&s), (PEOPLE as u64, 0, 0, 1));
        assert!(
            write < std::time::Duration::from_secs(1),
            "write took {write:?}"
        );
        assert!(
            read < std::time::Duration::from_secs(2),
            "open took {read:?}"
        );
    }
}

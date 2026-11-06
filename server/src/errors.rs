//! The error log: `<data>/errors/errors.jsonl`, one JSON line per failed conversion.
//!
//! It keeps what the engine said after the file and the report are gone, for 90 days. A line never
//! holds a file name, a path or an address: the engine's message goes through `clean_message`
//! first. A failure to write it is logged and changes nothing else.

use crate::engine::looks_like_path;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

const LOG_FILE: &str = "errors.jsonl";
const KEEP_DAYS: i64 = 90;
const MAX_MESSAGE_CHARS: usize = 1000;
const PATH_MARK: &str = "[path]";
const FILE_MARK: &str = "[file]";

/// One line of the log. `report`, `message` and `exit_code` are missing when there is none: a time
/// limit has no exit code, and an engine that crashed may have said nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorLine {
    pub at: DateTime<Utc>,
    pub report: Option<String>,
    pub kind: String,
    pub message: Option<String>,
    pub exit_code: Option<i32>,
    pub engine_version: String,
    pub engine_build: String,
    pub file_kind: String,
    pub year: u16,
    pub target: u16,
}

/// The log file of one data directory.
pub struct ErrorLog {
    path: PathBuf,
    /// The day of the last prune. Appending takes this lock too, so that no line written during a
    /// prune is lost.
    pruned: Mutex<Option<NaiveDate>>,
}

impl ErrorLog {
    /// The log under `dir`, created with its first line.
    pub fn new(dir: PathBuf) -> ErrorLog {
        ErrorLog {
            path: dir.join(LOG_FILE),
            pruned: Mutex::new(None),
        }
    }

    /// Adds a line. Touches the disk, so call it off the async threads.
    pub fn append(&self, line: &ErrorLine) {
        let _guard = self.lock();
        if let Err(e) = self.write_line(line) {
            tracing::warn!(error = ?e.kind(), "could not write the error log");
        }
    }

    fn write_line(&self, line: &ErrorLine) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut bytes = serde_json::to_vec(line).map_err(io::Error::other)?;
        bytes.push(b'\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(&bytes)
    }

    /// Drops the lines older than 90 days, at most once a day.
    pub fn prune(&self, now: DateTime<Utc>) {
        let mut pruned = self.lock();
        let today = now.date_naive();
        if *pruned == Some(today) {
            return;
        }
        match self.rewrite(now) {
            Ok(()) => *pruned = Some(today),
            Err(e) => tracing::warn!(error = ?e.kind(), "could not prune the error log"),
        }
    }

    /// Replaces the file through a temporary copy, so that it is always whole. A line that cannot
    /// be dated is dropped: nothing could ever age it out.
    fn rewrite(&self, now: DateTime<Utc>) -> io::Result<()> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        let text = String::from_utf8_lossy(&bytes);
        let keep = Duration::days(KEEP_DAYS);
        let kept: Vec<&str> = text
            .lines()
            .filter(|l| {
                serde_json::from_str::<ErrorLine>(l).is_ok_and(|line| now - line.at <= keep)
            })
            .collect();
        if kept.len() == text.lines().count() {
            return Ok(());
        }
        let mut out = kept.join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        let tmp = self.path.with_extension("jsonl.tmp");
        fs::write(&tmp, out)?;
        fs::rename(&tmp, &self.path)
    }

    fn lock(&self) -> MutexGuard<'_, Option<NaiveDate>> {
        self.pruned.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Makes an engine message safe to keep: the job's original file name is removed, the data
/// directory, every path and every Revit file name are replaced, and the last 1000 characters are
/// kept, since the real reason comes last. Cutting comes after cleaning, so that it never leaves
/// the end of a path no longer recognised as one. A path can hold spaces, so it runs to its
/// closing quote, to the next `: `, or else to the end of the line.
pub fn clean_message(raw: &str, data_dir: &Path, file_name: &str) -> String {
    let mut text = raw.to_string();
    if !file_name.is_empty() {
        text = text.replace(file_name, FILE_MARK);
    }
    let dir = data_dir.to_string_lossy();
    if !dir.is_empty() {
        text = text.replace(dir.as_ref(), PATH_MARK);
    }
    let lines: Vec<String> = text.split('\n').map(clean_line).collect();
    let text = lines.join("\n");
    let extra = text.chars().count().saturating_sub(MAX_MESSAGE_CHARS);
    text.chars().skip(extra).collect()
}

fn clean_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    let mut before = None;
    while let Some(c) = rest.chars().next() {
        if before.is_none_or(opens_path) && starts_path(rest) {
            let after_mark = out.ends_with(PATH_MARK);
            out.push_str(PATH_MARK);
            rest = &rest[path_end(rest, before, after_mark)..];
            before = Some('/');
            continue;
        }
        out.push(c);
        before = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    let doubled = PATH_MARK.repeat(2);
    while out.contains(&doubled) {
        out = out.replace(&doubled, PATH_MARK);
    }
    clean_words(&out)
}

/// What is left can only be a relative path or a bare file name. A relative path can hold spaces
/// too: it runs to the word that names its file, when one follows. Where the engine's own source
/// panicked stays: it is no file of a user, and all that tells a panic apart.
fn clean_words(line: &str) -> String {
    let words: Vec<&str> = line.split(' ').collect();
    let mut kept = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let word = words[i];
        if is_source_location(word) {
            kept.push(word);
        } else if looks_like_path(word) {
            if !names_revit_file(word)
                && let Some(n) = words[i + 1..].iter().position(|w| names_revit_file(w))
            {
                i += n + 1;
            }
            kept.push(PATH_MARK);
        } else if names_revit_file(word) {
            kept.push(FILE_MARK);
        } else {
            kept.push(word);
        }
        i += 1;
    }
    kept.join(" ")
}

/// `crates\core\src\translate.rs:429:17:`, as a panic names it.
fn is_source_location(word: &str) -> bool {
    let mut parts = word.trim_end_matches(':').rsplitn(3, ':');
    let number = |p: Option<&str>| p.is_some_and(|n| n.parse::<u32>().is_ok());
    number(parts.next()) && number(parts.next()) && parts.next().is_some_and(|f| f.ends_with(".rs"))
}

fn names_revit_file(word: &str) -> bool {
    let w = word
        .trim_matches(|c: char| "\"'`()[]{}<>,;:.".contains(c))
        .to_ascii_lowercase();
    [".rvt", ".rfa", ".rte", ".rft"]
        .iter()
        .any(|ext| w.ends_with(ext))
}

const QUOTES: &str = "\"'`";

/// Where the path at the start of `rest` ends, so that what the engine says after it survives:
/// `<path>: <reason>` is how it reports a file it cannot read. After a quote, the last such quote
/// ends it, since an apostrophe can be part of a name; taking too much is the safe side. What
/// follows the data directory's mark is a job's own path, which has no spaces.
fn path_end(rest: &str, before: Option<char>, after_mark: bool) -> usize {
    if let Some(end) = before
        .filter(|q| QUOTES.contains(*q))
        .and_then(|q| rest.rfind(q))
    {
        return end;
    }
    let reason = rest.find(": ").unwrap_or(rest.len());
    if after_mark {
        let word = rest.find(char::is_whitespace).unwrap_or(rest.len());
        return reason.min(word);
    }
    reason
}

/// A path can start right after this character.
fn opens_path(c: char) -> bool {
    c.is_whitespace() || QUOTES.contains(c) || "([{<=,:]".contains(c)
}

fn starts_path(s: &str) -> bool {
    let b = s.as_bytes();
    let after_slash = s.get(1..).and_then(|t| t.chars().next());
    let separator = |c: Option<&u8>| matches!(c, Some(b'\\' | b'/'));
    let name = |c: Option<&u8>| c.is_some_and(|c| c.is_ascii_alphanumeric() || b"._~".contains(c));
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && separator(b.get(2));
    drive
        || b.first() == Some(&b'\\')
        || (b.first() == Some(&b'/')
            && (name(b.get(1)) || after_slash.is_some_and(|c| !c.is_ascii())))
        || (b.first() == Some(&b'~') && separator(b.get(1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};
    use std::path::Path;

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, day, 12, 0, 0).unwrap()
    }

    fn line(at: DateTime<Utc>) -> ErrorLine {
        ErrorLine {
            at,
            report: Some("R-AAAAAA".into()),
            kind: "failed".into(),
            message: Some("The conversion failed".into()),
            exit_code: Some(5),
            engine_version: "engine 1.0".into(),
            engine_build: "abc".into(),
            file_kind: "rvt".into(),
            year: 2026,
            target: 2024,
        }
    }

    fn lines(dir: &Path) -> Vec<serde_json::Value> {
        std::fs::read_to_string(dir.join("errors.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn windows_path_with_spaces_is_replaced() {
        assert_eq!(
            clean_message(
                r"Cannot open C:\Users\Some One\My Models\tower.rvt: access denied",
                Path::new("D:/data"),
                "x.rvt"
            ),
            "Cannot open [path]: access denied"
        );
    }

    #[test]
    fn unix_path_with_spaces_is_replaced() {
        assert_eq!(
            clean_message(
                "Cannot open /home/some one/my models/tower.rvt\nstopped",
                Path::new("/srv/data"),
                "x.rvt"
            ),
            "Cannot open [path]\nstopped"
        );
    }

    #[test]
    fn a_quoted_path_ends_at_its_quote() {
        assert_eq!(
            clean_message(
                r#"Missing "C:\a b\c d.rfa" in the model and '/x y/z' too"#,
                Path::new("D:/data"),
                "x.rvt"
            ),
            r#"Missing "[path]" in the model and '[path]' too"#
        );
    }

    #[test]
    fn an_apostrophe_inside_a_quoted_path_does_not_end_it() {
        assert_eq!(
            clean_message(
                r"Missing 'C:\Bob's Final Models\t.rvt' in the model",
                Path::new("D:/data"),
                "x.rvt"
            ),
            "Missing '[path]' in the model"
        );
    }

    #[test]
    fn the_reason_after_the_input_path_is_kept() {
        assert_eq!(
            clean_message(
                r"engine probe: C:\Data Dir\jobs\ab12\in.rvt: not an OLE file",
                Path::new(r"C:\Data Dir"),
                "x.rvt"
            ),
            "engine probe: [path]: not an OLE file"
        );
        assert_eq!(
            clean_message(
                r"Read C:\Data Dir\jobs\ab12\in.rvt failed at 12",
                Path::new(r"C:\Data Dir"),
                "x.rvt"
            ),
            "Read [path] failed at 12"
        );
    }

    #[test]
    fn the_data_directory_is_replaced_with_what_follows_it() {
        assert_eq!(
            clean_message(
                r"Bad stream in C:\Data Dir\jobs\ab12\in.rvt: offset 12",
                Path::new(r"C:\Data Dir"),
                "x.rvt"
            ),
            "Bad stream in [path]: offset 12"
        );
        assert_eq!(
            clean_message(
                "Bad stream in data/jobs/ab12/in.rvt",
                Path::new("data"),
                "x.rvt"
            ),
            "Bad stream in [path]"
        );
    }

    #[test]
    fn relative_paths_are_replaced() {
        assert_eq!(
            clean_message(r"Read jobs\ab\in.rvt failed", Path::new("/srv"), "x.rvt"),
            "Read [path] failed"
        );
    }

    #[test]
    fn the_original_file_name_is_removed() {
        assert_eq!(
            clean_message(
                "Family in Secret Tower v2.rvt is broken",
                Path::new("/srv/data"),
                "Secret Tower v2.rvt"
            ),
            "Family in [file] is broken"
        );
    }

    #[test]
    fn a_message_without_a_path_stays_as_it_is() {
        let text = "Unknown class 2026/2024 at a / b: Wall (id 12), it's broken";
        assert_eq!(clean_message(text, Path::new("/srv/data"), "x.rvt"), text);
    }

    #[test]
    fn a_slash_between_words_is_not_a_path() {
        for text in [
            "element thêm lại: chèn 3 / 5 record vào partition",
            "quá sâu / quá nhiều giá trị",
        ] {
            assert_eq!(clean_message(text, Path::new("/srv/data"), "x.rvt"), text);
        }
        assert_eq!(
            clean_message("Mở /thư mục/tệp lỗi", Path::new("/srv/data"), "x.rvt"),
            "Mở [path]"
        );
    }

    #[test]
    fn a_path_right_after_a_bracket_keeps_its_spaces() {
        assert_eq!(
            clean_message(
                r"[warn]C:\Some Private Client\Tower Fit Out\x.rvt: missing",
                Path::new("D:/data"),
                "y.rvt"
            ),
            "[warn][path]: missing"
        );
    }

    #[test]
    fn a_relative_path_runs_to_its_file_name() {
        assert_eq!(
            clean_message(
                r"Missing ..\Links\Site Plan A.rvt in the model",
                Path::new("/srv"),
                "x.rvt"
            ),
            "Missing [path] in the model"
        );
    }

    #[test]
    fn where_the_engine_panicked_stays() {
        let text = r"thread 'worker' (4812) panicked at crates\core\src\translate.rs:429:17:";
        assert_eq!(clean_message(text, Path::new("/srv"), "x.rvt"), text);
    }

    #[test]
    fn a_bare_revit_file_name_is_replaced() {
        assert_eq!(
            clean_message(
                "Missing link Tower-Arch.RVT, family Door.rfa.",
                Path::new("/srv"),
                "x.rvt"
            ),
            "Missing link [file] family [file]"
        );
    }

    #[test]
    fn cutting_comes_after_cleaning_and_keeps_the_end() {
        let path = r"C:\Users\Acme Hospital Project\Secret Tower v2.rvt: access denied";
        let text = format!("{path}\n{}\nthe real reason", "x".repeat(975));
        let clean = clean_message(&text, Path::new("/srv"), "x.rvt");
        assert_eq!(clean.chars().count(), 1000);
        assert!(clean.ends_with("the real reason"), "{clean}");
        for leak in ["Acme", "Tower", "v2"] {
            assert!(!clean.contains(leak), "{clean}");
        }
    }

    #[test]
    fn a_message_is_cut_at_1000_characters() {
        let long = "é".repeat(1500);
        assert_eq!(
            clean_message(&long, Path::new("/srv/data"), "x.rvt")
                .chars()
                .count(),
            1000
        );
    }

    #[test]
    fn append_writes_one_line_with_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let log = ErrorLog::new(dir.path().join("errors"));
        log.append(&line(at(6)));
        log.append(&ErrorLine {
            report: None,
            message: None,
            exit_code: None,
            ..line(at(6))
        });
        let all = lines(&dir.path().join("errors"));
        assert_eq!(all.len(), 2);
        assert_eq!(all[0]["at"], "2026-11-06T12:00:00Z");
        assert_eq!(all[0]["report"], "R-AAAAAA");
        assert_eq!(all[0]["kind"], "failed");
        assert_eq!(all[0]["message"], "The conversion failed");
        assert_eq!(all[0]["exit_code"], 5);
        assert_eq!(all[0]["engine_version"], "engine 1.0");
        assert_eq!(all[0]["engine_build"], "abc");
        assert_eq!(all[0]["file_kind"], "rvt");
        assert_eq!(all[0]["year"], 2026);
        assert_eq!(all[0]["target"], 2024);
        assert_eq!(all[1]["report"], serde_json::Value::Null);
        assert_eq!(all[1]["exit_code"], serde_json::Value::Null);
    }

    #[test]
    fn a_failed_write_changes_nothing_for_the_caller() {
        let dir = tempfile::tempdir().unwrap();
        // A file where the directory should go.
        let blocked = dir.path().join("errors");
        std::fs::write(&blocked, b"").unwrap();
        ErrorLog::new(blocked).append(&line(at(6)));
    }

    #[test]
    fn prune_drops_lines_older_than_90_days() {
        let dir = tempfile::tempdir().unwrap();
        let log = ErrorLog::new(dir.path().to_path_buf());
        let now = at(6);
        log.append(&line(now - Duration::days(90) - Duration::seconds(1)));
        log.append(&line(now - Duration::days(90)));
        log.append(&line(now - Duration::days(1)));
        log.prune(now);
        assert_eq!(lines(dir.path()).len(), 2);
        assert!(!dir.path().join("errors.jsonl.tmp").exists());
    }

    #[test]
    fn prune_runs_once_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let log = ErrorLog::new(dir.path().to_path_buf());
        let now = at(6);
        log.prune(now);
        log.append(&line(now - Duration::days(100)));
        log.prune(now + Duration::hours(1));
        assert_eq!(lines(dir.path()).len(), 1, "same day: nothing happens");
        log.prune(now + Duration::days(1));
        assert_eq!(lines(dir.path()).len(), 0);
    }

    #[test]
    fn prune_without_a_file_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        ErrorLog::new(dir.path().join("errors")).prune(at(6));
        assert!(!dir.path().join("errors").exists());
    }
}

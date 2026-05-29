//! Execution history   per-spec append-only log stored under `.trellis/history/`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One execution history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// Whether the execution passed.
    pub passed: bool,
    /// HTTP status code when available.
    pub status: Option<u16>,
    /// Execution duration in milliseconds.
    pub duration_ms: u128,
    /// Structured error type when execution failed.
    pub error_type: Option<String>,
}

/// Return the history file path for a spec.
///
/// `spec_rel` is relative to the collection root, e.g. `"apis/users/create-user.md"`.
pub fn history_path(collection_root: &Path, spec_rel: &str) -> PathBuf {
    let slug = spec_rel
        .trim_end_matches(".md")
        .replace(['/', '\\', ' '], "-");
    collection_root
        .join(".trellis")
        .join("history")
        .join(format!("{slug}.json"))
}

/// Append an entry to the spec's history file, keeping the last 20 entries.
///
/// Best-effort: silently ignores all I/O errors.
pub fn write_entry(collection_root: &Path, spec_rel: &str, entry: HistoryEntry) {
    let path = history_path(collection_root, spec_rel);
    let mut entries = read_history(collection_root, spec_rel);
    entries.push(entry);
    // Keep the most recent 20.
    if entries.len() > 20 {
        let drop = entries.len() - 20;
        entries.drain(..drop);
    }
    // Best-effort write   ignore errors.
    let _ = (|| -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&entries).unwrap_or_default();
        std::fs::write(&path, json)
    })();
}

/// Read the history for a spec. Returns an empty vec on any error.
pub fn read_history(collection_root: &Path, spec_rel: &str) -> Vec<HistoryEntry> {
    let path = history_path(collection_root, spec_rel);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Compute a trend string from recent history.
///
/// Compares the pass rate of the last 3 entries against the previous 3.
/// Returns `"stable"`, `"improving"`, or `"regressing"`.
pub fn compute_trend(entries: &[HistoryEntry]) -> &'static str {
    if entries.len() < 4 {
        return "stable";
    }
    let len = entries.len();
    let recent_start = len.saturating_sub(3);
    let prev_end = recent_start;
    let prev_start = prev_end.saturating_sub(3);

    let pass_rate = |slice: &[HistoryEntry]| -> f64 {
        if slice.is_empty() {
            return 0.0;
        }
        let passed = slice.iter().filter(|e| e.passed).count();
        passed as f64 / slice.len() as f64
    };

    let recent = pass_rate(&entries[recent_start..]);
    let prev = pass_rate(&entries[prev_start..prev_end]);

    if recent > prev + 0.01 {
        "improving"
    } else if recent < prev - 0.01 {
        "regressing"
    } else {
        "stable"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_path_generates_slug() {
        let root = Path::new("/tmp/api-docs");
        let p = history_path(root, "apis/users/create-user.md");
        assert_eq!(
            p,
            PathBuf::from("/tmp/api-docs/.trellis/history/apis-users-create-user.json")
        );
    }

    #[test]
    fn write_and_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let entry = HistoryEntry {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            passed: true,
            status: Some(200),
            duration_ms: 123,
            error_type: None,
        };
        write_entry(dir.path(), "apis/test.md", entry.clone());
        let entries = read_history(dir.path(), "apis/test.md");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].passed, true);
        assert_eq!(entries[0].duration_ms, 123);
    }

    #[test]
    fn caps_at_twenty_entries() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..25u32 {
            let entry = HistoryEntry {
                timestamp: format!("2024-01-{:02}T00:00:00Z", i + 1),
                passed: true,
                status: Some(200),
                duration_ms: u128::from(i),
                error_type: None,
            };
            write_entry(dir.path(), "test.md", entry);
        }
        let entries = read_history(dir.path(), "test.md");
        assert_eq!(entries.len(), 20);
        // Most recent entry should be the last written (duration 24).
        assert_eq!(entries.last().unwrap().duration_ms, 24);
    }

    #[test]
    fn trend_stable_when_few_entries() {
        let entries: Vec<HistoryEntry> = vec![];
        assert_eq!(compute_trend(&entries), "stable");
    }

    #[test]
    fn trend_improving() {
        let make = |passed: bool| HistoryEntry {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            passed,
            status: Some(200),
            duration_ms: 10,
            error_type: None,
        };
        // 3 failures then 3 passes.
        let entries: Vec<_> = [false, false, false, true, true, true]
            .iter()
            .map(|&p| make(p))
            .collect();
        assert_eq!(compute_trend(&entries), "improving");
    }

    #[test]
    fn trend_regressing() {
        let make = |passed: bool| HistoryEntry {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            passed,
            status: Some(200),
            duration_ms: 10,
            error_type: None,
        };
        let entries: Vec<_> = [true, true, true, false, false, false]
            .iter()
            .map(|&p| make(p))
            .collect();
        assert_eq!(compute_trend(&entries), "regressing");
    }
}

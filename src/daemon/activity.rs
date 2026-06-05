//! Daemon-side activity log ring buffer.
//!
//! The pclsync event callback fires from internal library threads whenever a
//! file is downloaded/uploaded/deleted. The daemon captures those into a
//! bounded, sequence-tagged ring buffer so IPC clients (the TUI) can poll for
//! new entries incrementally via [`DaemonCommand::ActivitySince`].
//!
//! [`DaemonCommand::ActivitySince`]: super::ipc::DaemonCommand::ActivitySince

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::wrapper::ActivityEntry;

/// Maximum number of activity entries retained in the ring buffer.
const MAX_ACTIVITY: usize = 200;

/// Thread-safe, bounded, sequence-tagged activity log.
///
/// Shared between the pclsync event callback (producer, fires from C threads)
/// and the IPC server (consumer, answers `ActivitySince`). Lock scopes are kept
/// tiny so the callback never blocks the sync engine.
#[derive(Default)]
pub struct ActivityLog {
    entries: Mutex<VecDeque<ActivityEntry>>,
    next_seq: AtomicU64,
}

impl ActivityLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a new entry, assigning it the next sequence id and trimming the
    /// oldest entry when the buffer is full.
    pub fn push(&self, timestamp: String, description: String, is_error: bool) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let entry = ActivityEntry {
            seq,
            timestamp,
            description,
            is_error,
        };
        if let Ok(mut buf) = self.entries.lock() {
            buf.push_back(entry);
            while buf.len() > MAX_ACTIVITY {
                buf.pop_front();
            }
        }
    }

    /// Return all entries with `seq > cursor`, plus the new cursor (the highest
    /// seq returned, or the input cursor when nothing is new).
    pub fn since(&self, cursor: u64) -> (Vec<ActivityEntry>, u64) {
        let buf = match self.entries.lock() {
            Ok(b) => b,
            Err(_) => return (Vec::new(), cursor),
        };
        let entries: Vec<ActivityEntry> = buf.iter().filter(|e| e.seq > cursor).cloned().collect();
        let new_cursor = entries.last().map(|e| e.seq).unwrap_or(cursor);
        (entries, new_cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_returns_only_newer_entries() {
        let log = ActivityLog::new();
        log.push("00:00:01".into(), "a".into(), false);
        log.push("00:00:02".into(), "b".into(), false);

        let (all, cursor) = log.since(0);
        assert_eq!(all.len(), 2);
        assert_eq!(cursor, 2);

        log.push("00:00:03".into(), "c".into(), true);
        let (newer, cursor2) = log.since(cursor);
        assert_eq!(newer.len(), 1);
        assert_eq!(newer[0].description, "c");
        assert!(newer[0].is_error);
        assert_eq!(cursor2, 3);

        // Polling again with the latest cursor yields nothing new.
        let (none, cursor3) = log.since(cursor2);
        assert!(none.is_empty());
        assert_eq!(cursor3, cursor2);
    }

    #[test]
    fn ring_buffer_is_bounded() {
        let log = ActivityLog::new();
        for i in 0..(MAX_ACTIVITY + 50) {
            log.push("t".into(), format!("e{i}"), false);
        }
        let (all, _) = log.since(0);
        assert_eq!(all.len(), MAX_ACTIVITY);
        // Oldest entries were dropped; the newest survives.
        assert_eq!(
            all.last().unwrap().description,
            format!("e{}", MAX_ACTIVITY + 49)
        );
    }
}

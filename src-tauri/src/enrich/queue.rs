//! The persistent enrichment queue, SPEC 5.6.
//!
//! Jobs outlive restarts, so a note captured while the model was absent still
//! gets labelled later. The queue itself is only bookkeeping: the note's
//! `status` in the markdown is what the user sees, and this file can be
//! deleted without losing a note.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// SPEC 5.6: three attempts, then the note is marked failed.
pub const MAX_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    /// Carried so a job can find its file without searching every day.
    pub date: String,
    #[serde(default)]
    pub attempts: u32,
}

impl Job {
    pub fn new(id: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            date: date.into(),
            attempts: 0,
        }
    }

    /// How long to wait before the next try. Doubles each time.
    pub fn backoff_secs(&self) -> u64 {
        2u64.saturating_pow(self.attempts)
    }

    pub fn exhausted(&self) -> bool {
        self.attempts >= MAX_ATTEMPTS
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Queue {
    jobs: VecDeque<Job>,
}

impl Queue {
    /// Adding a note already queued is a no-op, so a file saved twice in quick
    /// succession does not enrich twice.
    pub fn push(&mut self, job: Job) -> bool {
        if self.jobs.iter().any(|queued| queued.id == job.id) {
            return false;
        }
        self.jobs.push_back(job);
        true
    }

    /// FIFO, one at a time (SPEC 5.6).
    pub fn pop(&mut self) -> Option<Job> {
        self.jobs.pop_front()
    }

    /// Put a failed job back with its attempt counted. Returns false when it
    /// has run out of attempts and the note should be marked failed instead.
    pub fn requeue(&mut self, mut job: Job) -> bool {
        job.attempts += 1;
        if job.exhausted() {
            return false;
        }
        self.jobs.push_back(job);
        true
    }

    /// Put a job back at the front without counting an attempt, for when the
    /// note changed under us and the result was discarded rather than failed.
    pub fn requeue_unchanged(&mut self, job: Job) {
        self.jobs.push_front(job);
    }

    pub fn remove(&mut self, id: &str) {
        self.jobs.retain(|job| job.id != id);
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{\"jobs\":[]}".to_string())
    }

    /// An unreadable queue starts empty rather than blocking startup. Nothing
    /// is lost that a rescan of pending notes cannot restore.
    pub fn from_json(raw: &str) -> Self {
        serde_json::from_str(raw).unwrap_or_else(|e| {
            log::warn!("queue.json is not readable ({e}), starting empty");
            Self::default()
        })
    }

    pub fn load(root: &Path) -> Self {
        match std::fs::read_to_string(queue_path(root)) {
            Ok(raw) => Self::from_json(&raw),
            Err(_) => Self::default(),
        }
    }
}

pub fn queue_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("queue.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hands_jobs_back_in_the_order_they_arrived() {
        let mut queue = Queue::default();
        queue.push(Job::new("01AAA", "2026-09-22"));
        queue.push(Job::new("01BBB", "2026-09-22"));

        assert_eq!(queue.pop().unwrap().id, "01AAA");
        assert_eq!(queue.pop().unwrap().id, "01BBB");
        assert!(queue.pop().is_none());
    }

    #[test]
    fn refuses_to_queue_the_same_note_twice() {
        let mut queue = Queue::default();
        assert!(queue.push(Job::new("01AAA", "2026-09-22")));
        assert!(!queue.push(Job::new("01AAA", "2026-09-22")));
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn retries_up_to_three_attempts_then_gives_up() {
        let mut queue = Queue::default();
        queue.push(Job::new("01AAA", "2026-09-22"));

        for expected in 1..MAX_ATTEMPTS {
            let job = queue.pop().unwrap();
            assert!(queue.requeue(job), "should still have attempts left");
            assert_eq!(queue.jobs.front().unwrap().attempts, expected);
        }

        let last = queue.pop().unwrap();
        assert_eq!(last.attempts, MAX_ATTEMPTS - 1);
        assert!(!queue.requeue(last), "the third failure gives up");
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn backs_off_further_on_each_attempt() {
        let mut job = Job::new("01AAA", "2026-09-22");
        assert_eq!(job.backoff_secs(), 1);
        job.attempts = 1;
        assert_eq!(job.backoff_secs(), 2);
        job.attempts = 2;
        assert_eq!(job.backoff_secs(), 4);
    }

    #[test]
    fn a_discarded_result_goes_back_to_the_front_without_burning_an_attempt() {
        let mut queue = Queue::default();
        queue.push(Job::new("01AAA", "2026-09-22"));
        queue.push(Job::new("01BBB", "2026-09-22"));

        let job = queue.pop().unwrap();
        queue.requeue_unchanged(job);

        let next = queue.pop().unwrap();
        assert_eq!(next.id, "01AAA");
        assert_eq!(next.attempts, 0, "a changed note is not a failure");
    }

    #[test]
    fn a_deleted_note_drops_out_of_the_queue() {
        let mut queue = Queue::default();
        queue.push(Job::new("01AAA", "2026-09-22"));
        queue.push(Job::new("01BBB", "2026-09-22"));

        queue.remove("01AAA");
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pop().unwrap().id, "01BBB");
    }

    #[test]
    fn survives_a_round_trip_through_json() {
        let mut queue = Queue::default();
        queue.push(Job::new("01AAA", "2026-09-22"));
        let mut job = Job::new("01BBB", "2026-09-23");
        job.attempts = 2;
        queue.push(job);

        let back = Queue::from_json(&queue.to_json());
        assert_eq!(back.len(), 2);
        assert_eq!(back.jobs[1].attempts, 2);
        assert_eq!(back.jobs[1].date, "2026-09-23");
    }

    #[test]
    fn an_unreadable_queue_starts_empty_instead_of_failing() {
        assert_eq!(Queue::from_json("not json").len(), 0);
    }
}

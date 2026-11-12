//! Alert for a Canvas write transaction that holds the single writer too long
//! (docs/decisions/2026-10-28-target-client-server-architecture.md, "Storage").
//!
//! The owner of the duration is `SQLITE_WRITE_TRANSACTION_ALERT_MS` in
//! `studio_runtime_schema::shared_values`. Slow transactions log a rate-limited warning with
//! the caller label (`file:line` of the `transaction` call) and the duration. The
//! runtime diagnostics list the same transactions in `sqliteContention.slowTransactions`.

use std::collections::HashMap;
use std::panic::Location;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use studio_runtime_schema::shared_values::SQLITE_WRITE_TRANSACTION_ALERT_MS;

/// Minimum interval between warning lines at one transaction call site. Unit: seconds.
const WRITE_ALERT_INTERVAL_SECONDS: u64 = 60;

#[derive(Debug)]
struct Site {
    last_line: Instant,
    count: u64,
    max: Duration,
}

#[derive(Debug, PartialEq, Eq)]
struct Summary {
    count: u64,
    max: Duration,
}

#[derive(Debug)]
pub(crate) struct WriteAlert {
    threshold_ms: AtomicU64,
    alerts: AtomicU64,
    sites: Mutex<HashMap<&'static Location<'static>, Site>>,
}

impl Default for WriteAlert {
    fn default() -> Self {
        Self {
            threshold_ms: AtomicU64::new(SQLITE_WRITE_TRANSACTION_ALERT_MS),
            alerts: AtomicU64::new(0),
            sites: Mutex::default(),
        }
    }
}

impl WriteAlert {
    /// Log a warning when `held` reaches the threshold. Runs on the writer thread after the
    /// transaction ended, so it holds no lock of the database.
    pub(crate) fn observe(&self, origin: &'static Location<'static>, held: Duration) {
        if let Some(summary) = self.observe_at(origin, held, Instant::now()) {
            let threshold = Duration::from_millis(self.threshold_ms.load(Ordering::Relaxed));
            eprintln!(
                "{}; {} slow transactions since last warning, max {} ms",
                alert_line(origin, held, threshold),
                summary.count,
                summary.max.as_millis()
            );
        }
    }

    fn observe_at(
        &self,
        origin: &'static Location<'static>,
        held: Duration,
        now: Instant,
    ) -> Option<Summary> {
        let threshold = Duration::from_millis(self.threshold_ms.load(Ordering::Relaxed));
        if held < threshold {
            return None;
        }
        self.alerts.fetch_add(1, Ordering::Relaxed);
        let mut sites = self
            .sites
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let site = match sites.entry(origin) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Site {
                    last_line: now,
                    count: 0,
                    max: Duration::ZERO,
                });
                return Some(Summary {
                    count: 1,
                    max: held,
                });
            }
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        };
        site.count = site.count.saturating_add(1);
        site.max = site.max.max(held);
        if now.duration_since(site.last_line) < Duration::from_secs(WRITE_ALERT_INTERVAL_SECONDS) {
            return None;
        }
        let summary = Summary {
            count: site.count,
            max: site.max,
        };
        site.last_line = now;
        site.count = 0;
        site.max = Duration::ZERO;
        Some(summary)
    }

    pub(crate) fn count(&self) -> u64 {
        self.alerts.load(Ordering::Relaxed)
    }

    pub(crate) fn set_threshold(&self, threshold: Duration) {
        let millis = u64::try_from(threshold.as_millis()).unwrap_or(u64::MAX);
        self.threshold_ms.store(millis, Ordering::Relaxed);
    }
}

/// The warning line for one slow write transaction.
pub(crate) fn alert_line(origin: &Location<'_>, held: Duration, threshold: Duration) -> String {
    format!(
        "warning: Canvas write transaction held the writer for {} ms (alert at {} ms), caller {}:{}",
        held.as_millis(),
        threshold.as_millis(),
        origin.file(),
        origin.line()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_alert_default_is_one_hundred_milliseconds() {
        assert_eq!(SQLITE_WRITE_TRANSACTION_ALERT_MS, 100);
    }

    #[test]
    fn writer_alert_rate_limit_is_per_site_and_summarizes_the_window() {
        let alert = WriteAlert::default();
        alert.set_threshold(Duration::from_millis(100));
        let first = Location::caller();
        let other = Location::caller();
        let now = Instant::now();
        let held = Duration::from_millis(120);
        assert_eq!(
            alert.observe_at(first, held, now),
            Some(Summary {
                count: 1,
                max: held
            })
        );
        assert_eq!(
            alert.observe_at(
                first,
                Duration::from_millis(500),
                now + Duration::from_secs(1)
            ),
            None
        );
        assert_eq!(
            alert.observe_at(other, held, now),
            Some(Summary {
                count: 1,
                max: held
            })
        );
        assert_eq!(
            alert.observe_at(first, held, now + Duration::from_secs(59)),
            None
        );
        assert_eq!(
            alert.observe_at(
                first,
                held,
                now + Duration::from_secs(WRITE_ALERT_INTERVAL_SECONDS)
            ),
            Some(Summary {
                count: 3,
                max: Duration::from_millis(500)
            })
        );
        assert_eq!(alert.count(), 5);
    }

    #[tokio::test]
    async fn writer_alert_observes_commit_and_rollback_in_normal_builds() {
        let root = tempfile::tempdir().unwrap();
        let store = crate::RuntimeStore::open(Some(root.path())).await.unwrap();
        let before = store.write_alerts();
        store.set_write_alert_threshold(Duration::ZERO);
        store.transaction(|_| Ok(())).await.unwrap();
        assert_eq!(store.write_alerts(), before + 1);
        let result: crate::StoreResult<()> = store
            .transaction(|_| Err(crate::StoreError::RecordContract("rollback witness")))
            .await;
        assert!(result.is_err());
        assert_eq!(store.write_alerts(), before + 2);
    }

    #[test]
    fn only_a_transaction_at_the_threshold_alerts() {
        let alert = WriteAlert::default();
        alert.set_threshold(Duration::from_millis(50));
        let origin = Location::caller();
        alert.observe(origin, Duration::from_millis(49));
        assert_eq!(alert.count(), 0);
        alert.observe(origin, Duration::from_millis(50));
        assert_eq!(alert.count(), 1);
    }

    #[test]
    fn the_warning_names_the_caller_and_the_duration() {
        let line = alert_line(
            Location::caller(),
            Duration::from_millis(1234),
            Duration::from_millis(1000),
        );
        assert!(line.contains("held the writer for 1234 ms"), "{line}");
        assert!(line.contains("alert at 1000 ms"), "{line}");
        assert!(line.contains("write_alert.rs:"), "{line}");
    }
}

//! Adapter boundary clocks. Wall timestamps are Unix seconds; durations use
//! the host's CLOCK_MONOTONIC nanoseconds and never subtract wall clocks.
use serde::{Deserialize, Serialize};

/// Transport-owned receive stamp carried with the existing journal payload.
pub const BOUNDARY_STAMP_FIELD: &str = "_studioBoundaryAt";
/// Existing event metadata timing field recording server input admission.
pub const ACCEPTED_STAMP_FIELD: &str = "acceptedBoundary";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundaryStamp {
    /// Kernel boot UUID; absent on legacy stamps or when unavailable.
    #[serde(default)]
    pub boot_id: Option<uuid::Uuid>,
    #[schemars(range(min = 0))]
    pub unix_seconds: f64,
    pub monotonic_ns: u64,
}

impl BoundaryStamp {
    /// A missing boot UUID cannot establish a shared monotonic clock domain.
    pub fn same_boot(self, other: Self) -> bool {
        self.boot_id.is_some() && self.boot_id == other.boot_id
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderBoundaryTiming {
    pub request_id: Option<String>,
    pub provider_handle: Option<String>,
    pub generation: Option<u64>,
    pub enqueued: Option<BoundaryStamp>,
    pub accepted: Option<BoundaryStamp>,
    pub written: Option<BoundaryStamp>,
    pub first_event: Option<BoundaryStamp>,
    pub first_token: Option<BoundaryStamp>,
    pub terminal: Option<BoundaryStamp>,
    pub spawn_started: Option<BoundaryStamp>,
    pub initialized: Option<BoundaryStamp>,
    pub accepted_to_written_ms: Option<f64>,
    pub enqueued_to_accepted_ms: Option<f64>,
    pub written_to_first_event_ms: Option<f64>,
    pub written_to_first_token_ms: Option<f64>,
    pub accepted_to_terminal_ms: Option<f64>,
    pub spawn_to_initialized_ms: Option<f64>,
}

impl ProviderBoundaryTiming {
    /// A replay can arrive after a later event's projection. Keep the original
    /// transport occurrence, including when the wall clock has moved backward.
    pub fn merge_first(&mut self, incoming: Self) {
        if self.request_id.is_some()
            && incoming.request_id.is_some()
            && self.request_id != incoming.request_id
        {
            return;
        }
        self.request_id = self.request_id.take().or(incoming.request_id);
        self.provider_handle = self.provider_handle.take().or(incoming.provider_handle);
        self.generation = self.generation.or(incoming.generation);
        fn first(target: &mut Option<BoundaryStamp>, stamp: Option<BoundaryStamp>) {
            if let Some(stamp) = stamp
                && target
                    .is_none_or(|old| stamp.same_boot(old) && stamp.monotonic_ns < old.monotonic_ns)
            {
                *target = Some(stamp);
            }
        }
        first(&mut self.enqueued, incoming.enqueued);
        first(&mut self.accepted, incoming.accepted);
        first(&mut self.written, incoming.written);
        first(&mut self.first_event, incoming.first_event);
        first(&mut self.first_token, incoming.first_token);
        first(&mut self.terminal, incoming.terminal);
        first(&mut self.spawn_started, incoming.spawn_started);
        first(&mut self.initialized, incoming.initialized);
        self.derive_durations();
    }

    pub fn derive_durations(&mut self) {
        fn elapsed(start: Option<BoundaryStamp>, end: Option<BoundaryStamp>) -> Option<f64> {
            let (start, end) = (start?, end?);
            if !start.same_boot(end) {
                return None;
            }
            end.monotonic_ns
                .checked_sub(start.monotonic_ns)
                .map(|ns| ns as f64 / 1_000_000.)
        }
        self.accepted_to_written_ms = elapsed(self.accepted, self.written);
        self.enqueued_to_accepted_ms = elapsed(self.enqueued, self.accepted);
        self.written_to_first_event_ms = elapsed(self.written, self.first_event);
        self.written_to_first_token_ms = elapsed(self.written, self.first_token);
        self.accepted_to_terminal_ms = elapsed(self.accepted, self.terminal);
        self.spawn_to_initialized_ms = elapsed(self.spawn_started, self.initialized);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_durations_use_monotonic_order_and_leave_absent_output_unknown() {
        let stamp = |monotonic_ns, unix_seconds| {
            Some(BoundaryStamp {
                boot_id: Some(uuid::Uuid::from_u128(1)),
                monotonic_ns,
                unix_seconds,
            })
        };
        let mut timing = ProviderBoundaryTiming {
            accepted: stamp(1_000_000, 20.),
            written: stamp(3_000_000, 19.),
            first_event: stamp(4_000_000, 18.),
            terminal: stamp(8_000_000, 17.),
            ..Default::default()
        };
        timing.derive_durations();
        assert_eq!(timing.accepted_to_written_ms, Some(2.));
        assert_eq!(timing.written_to_first_event_ms, Some(1.));
        assert_eq!(timing.accepted_to_terminal_ms, Some(7.));
        assert_eq!(timing.written_to_first_token_ms, None);
        timing.written = stamp(0, 21.);
        timing.derive_durations();
        assert_eq!(timing.accepted_to_written_ms, None);
    }
    #[test]
    fn boundary_queue_duration_is_unknown_across_boots() {
        let mut timing = ProviderBoundaryTiming {
            enqueued: Some(BoundaryStamp {
                boot_id: Some(uuid::Uuid::from_u128(1)),
                unix_seconds: 1000.,
                monotonic_ns: 60_000_000_000,
            }),
            accepted: Some(BoundaryStamp {
                boot_id: Some(uuid::Uuid::from_u128(2)),
                unix_seconds: 2000.,
                monotonic_ns: 120_000_000_000,
            }),
            ..Default::default()
        };
        timing.derive_durations();
        assert_eq!(timing.enqueued_to_accepted_ms, None);
        timing.enqueued = timing.enqueued.map(|mut stamp| {
            stamp.boot_id = None;
            stamp
        });
        timing.derive_durations();
        assert_eq!(timing.enqueued_to_accepted_ms, None);
    }
}

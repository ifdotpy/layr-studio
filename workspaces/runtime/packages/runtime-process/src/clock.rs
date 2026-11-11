//! Monotonic time shared by local runtime event enqueue and provider stdin writes.

/// System `CLOCK_MONOTONIC` nanoseconds, matching Python's `time.monotonic_ns` domain.
#[must_use]
pub fn monotonic_ns() -> u128 {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    time.tv_sec as u128 * 1_000_000_000 + time.tv_nsec as u128
}

/// Paired boundary sample. Wall time is for display, never elapsed arithmetic.
pub fn boundary_stamp() -> studio_runtime_schema::provider_timing::BoundaryStamp {
    studio_runtime_schema::provider_timing::BoundaryStamp {
        boot_id: crate::supervisor_identity::boot_id()
            .ok()
            .and_then(|boot| boot.parse().ok()),
        unix_seconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
        monotonic_ns: u64::try_from(monotonic_ns()).unwrap_or(u64::MAX),
    }
}

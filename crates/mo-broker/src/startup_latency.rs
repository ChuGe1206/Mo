//! Fixed startup metadata, enabled only by debug `latency-trace` builds.
//! Durations are buffered until readiness; paths, inputs and settings never enter this module.

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Phase {
    Parse,
    PipeBind,
    Settings,
    EngineLoad,
    BackendPrepare,
    EngineStart,
    WorkersStart,
    MainToReady,
}

#[cfg(all(debug_assertions, feature = "latency-trace"))]
impl Phase {
    pub(crate) const ALL: [Self; 8] = [
        Self::Parse,
        Self::PipeBind,
        Self::Settings,
        Self::EngineLoad,
        Self::BackendPrepare,
        Self::EngineStart,
        Self::WorkersStart,
        Self::MainToReady,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::PipeBind => "pipe_bind",
            Self::Settings => "settings",
            Self::EngineLoad => "engine_load",
            Self::BackendPrepare => "backend_prepare",
            Self::EngineStart => "engine_start",
            Self::WorkersStart => "workers_start",
            Self::MainToReady => "main_to_ready",
        }
    }
}

pub struct Span {
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    phase: Phase,
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    started: std::time::Instant,
}

#[cfg(not(all(debug_assertions, feature = "latency-trace")))]
const _: () = assert!(std::mem::size_of::<Span>() == 0);

impl Span {
    pub fn new(_phase: Phase) -> Self {
        Self {
            #[cfg(all(debug_assertions, feature = "latency-trace"))]
            phase: _phase,
            #[cfg(all(debug_assertions, feature = "latency-trace"))]
            started: std::time::Instant::now(),
        }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        #[cfg(all(debug_assertions, feature = "latency-trace"))]
        diagnostic::TIMINGS.record(self.phase, self.started.elapsed().as_micros());
    }
}

/// Call once at binary main entry. This excludes OS process creation and PE loading.
pub fn begin() {
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    diagnostic::ENTRY.get_or_init(std::time::Instant::now);
}

pub(crate) fn capture_ready() {
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    if let Some(entry) = diagnostic::ENTRY.get() {
        diagnostic::TIMINGS.record(Phase::MainToReady, entry.elapsed().as_micros());
    }
}

/// Called after the readiness line and logger initialization, with bounded try_send only.
pub(crate) fn publish() {
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    if diagnostic::ENTRY.get().is_some() {
        for phase in Phase::ALL {
            if let Some(elapsed_us) = diagnostic::TIMINGS.read(phase) {
                crate::latency::startup_record(phase, elapsed_us);
            }
        }
    }
}

#[cfg(all(debug_assertions, feature = "latency-trace"))]
mod diagnostic {
    use super::Phase;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    pub(super) static ENTRY: OnceLock<Instant> = OnceLock::new();
    pub(super) static TIMINGS: Timings = Timings::new();

    // One duration per fixed phase. These represent one Broker startup per process.
    pub(super) struct Timings([AtomicU64; 8]);
    impl Timings {
        const fn new() -> Self {
            Self([const { AtomicU64::new(0) }; 8])
        }
        pub(super) fn record(&self, phase: Phase, micros: u128) {
            let encoded = u64::try_from(micros).unwrap_or(u64::MAX).saturating_add(1);
            self.0[phase as usize].store(encoded, Ordering::Release);
        }
        pub(super) fn read(&self, phase: Phase) -> Option<u64> {
            self.0[phase as usize]
                .load(Ordering::Acquire)
                .checked_sub(1)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn absent_zero_and_saturated_durations_remain_distinct() {
            let timings = Timings::new();
            assert!(timings.read(Phase::Parse).is_none());
            timings.record(Phase::Parse, 0);
            assert_eq!(timings.read(Phase::Parse), Some(0));
            timings.record(Phase::EngineLoad, u128::MAX);
            assert_eq!(timings.read(Phase::EngineLoad), Some(u64::MAX - 1));
            assert!(timings.read(Phase::BackendPrepare).is_none());
        }
    }
}

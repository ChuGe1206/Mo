//! Opt-in development metadata only. No keys, text, paths or session tokens.

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    Create,
    Dispatch,
    Destroy,
}

pub(crate) struct Queued {
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    submitted: std::time::Instant,
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    operation: Operation,
}

pub(crate) struct Running {
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    queued: Queued,
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    started: std::time::Instant,
}

// Enforced in default AND release feature builds, not just by debug tests.
#[cfg(not(all(debug_assertions, feature = "latency-trace")))]
const _: () = {
    assert!(std::mem::size_of::<Queued>() == 0);
    assert!(std::mem::size_of::<Running>() == 0);
};

impl Queued {
    pub(crate) fn new(_operation: Operation) -> Self {
        Self {
            #[cfg(all(debug_assertions, feature = "latency-trace"))]
            submitted: std::time::Instant::now(),
            #[cfg(all(debug_assertions, feature = "latency-trace"))]
            operation: _operation,
        }
    }
    pub(crate) fn begin(self) -> Running {
        Running {
            #[cfg(all(debug_assertions, feature = "latency-trace"))]
            queued: self,
            #[cfg(all(debug_assertions, feature = "latency-trace"))]
            started: std::time::Instant::now(),
        }
    }
}

impl Running {
    pub(crate) fn finish(self) {
        #[cfg(all(debug_assertions, feature = "latency-trace"))]
        diagnostic::emit(diagnostic::Record {
            operation: self.queued.operation,
            queue_us: self
                .started
                .duration_since(self.queued.submitted)
                .as_micros(),
            engine_us: self.started.elapsed().as_micros(),
        });
    }
}

pub(crate) fn initialize() {
    #[cfg(all(debug_assertions, feature = "latency-trace"))]
    diagnostic::initialize();
}

#[cfg(all(debug_assertions, feature = "latency-trace"))]
mod diagnostic {
    use super::Operation;
    use std::io::{self, Write};
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};

    const CAPACITY: usize = 256;
    static SENDER: OnceLock<SyncSender<Record>> = OnceLock::new();
    static DROPPED: AtomicU64 = AtomicU64::new(0);
    pub(super) struct Record {
        pub operation: Operation,
        pub queue_us: u128,
        pub engine_us: u128,
    }

    pub(super) fn initialize() {
        SENDER.get_or_init(|| {
            let (sender, receiver) = sync_channel::<Record>(CAPACITY);
            // This optional logger owns no engine/session/pipe. Never join it:
            // stderr may block, but engine workers must remain independent.
            let _ = std::thread::Builder::new()
                .name("mo-latency-log".to_owned())
                .spawn(move || {
                    let mut stderr = io::stderr();
                    for record in receiver {
                        if write_record(&mut stderr, &record, DROPPED.load(Ordering::Relaxed))
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            sender
        });
    }

    fn write_record(writer: &mut impl Write, record: &Record, dropped: u64) -> io::Result<()> {
        let operation = match record.operation {
            Operation::Create => "create",
            Operation::Dispatch => "dispatch",
            Operation::Destroy => "destroy",
        };
        writeln!(
            writer,
            "MO_LATENCY op={operation} queue_us={} engine_us={} dropped={dropped}",
            record.queue_us, record.engine_us
        )
    }

    fn try_record(sender: &SyncSender<Record>, record: Record) -> bool {
        match sender.try_send(record) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
        }
    }

    pub(super) fn emit(record: Record) {
        // Ready is published before starting the optional logger so its output
        // cannot precede the readiness line. Count early records as dropped too.
        if !SENDER
            .get()
            .is_some_and(|sender| try_record(sender, record))
        {
            let _ = DROPPED.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                Some(count.saturating_add(1))
            });
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn record() -> Record {
            Record {
                operation: Operation::Dispatch,
                queue_us: 12,
                engine_us: 34,
            }
        }
        #[test]
        fn full_or_disconnected_logger_never_blocks_or_panics() {
            let (sender, receiver) = sync_channel(1);
            assert!(try_record(&sender, record()));
            let started = std::time::Instant::now();
            assert!(!try_record(&sender, record()));
            assert!(started.elapsed() < std::time::Duration::from_millis(100));
            drop(receiver);
            assert!(!try_record(&sender, record()));
        }
        #[test]
        fn output_has_only_fixed_metadata_fields() {
            let mut output = Vec::new();
            write_record(&mut output, &record(), 7).unwrap();
            assert_eq!(
                String::from_utf8(output).unwrap(),
                "MO_LATENCY op=dispatch queue_us=12 engine_us=34 dropped=7\n"
            );
        }
    }
}

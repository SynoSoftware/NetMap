use super::{ConnectionCommand, ConnectionUpdate, Reporter};
use crate::{
    app::exploration::Rows,
    connections::{Collector, Highlight, etw, pipe},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use windows::core::GUID;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Local(u64),
    Broker(GUID),
}

type Latest = Arc<Mutex<Option<Result<ConnectionUpdate, String>>>>;

/// One worker chooses the available source. Both roles use the same Collector;
/// switching source retires old identities and never merges measurement clocks.
pub(super) fn run(
    mut highlight: Highlight,
    requests: mpsc::Receiver<ConnectionCommand>,
    consumers: mpsc::SyncSender<(u64, etw::Consumer)>,
    latest: Latest,
    reporter: Reporter,
    retired: Arc<AtomicBool>,
) {
    let mut collector: Option<Collector> = None;
    let mut client: Option<pipe::Client> = None;
    let mut generation = 0u64;
    let mut source = None;
    let mut last_broker = None;
    let mut rows: Option<Arc<Rows>> = None;
    let mut next_probe = Instant::now();
    let mut next_sample = Instant::now();
    while !retired.load(Ordering::Acquire) {
        while let Ok(command) = requests.try_recv() {
            command.apply(&mut highlight, &mut rows, source, collector.as_mut());
        }
        if client.is_none() && Instant::now() >= next_probe {
            next_probe = Instant::now() + Duration::from_secs(5);
            if let Ok(connected) = pipe::Client::connect(&retired) {
                // Drop the owned local trace before subscribing to the shared
                // source. Its consumer can finish on its registered worker.
                collector.take();
                client = Some(connected);
                rows = None;
            }
        }
        if let Some(connected) = client.as_mut() {
            match connected.read(&retired) {
                Ok(received) => {
                    #[cfg(test)]
                    crate::host::native_broker::received(received.epoch, received.sequence);
                    if last_broker.is_some_and(|(epoch, sequence)| {
                        epoch == received.epoch && sequence >= received.sequence
                    }) {
                        continue;
                    }
                    last_broker = Some((received.epoch, received.sequence));
                    let current = Source::Broker(received.epoch);
                    if source != Some(current) {
                        rows = None;
                    }
                    source = Some(current);
                    let update =
                        ConnectionUpdate::prepare(received.publication, current, rows.as_deref());
                    publish(&latest, &reporter, Ok(update));
                }
                Err(error) => {
                    client.take();
                    publish(
                        &latest,
                        &reporter,
                        Err(format!("Shared collection disconnected: {error}")),
                    );
                    next_sample = Instant::now();
                }
            }
            continue;
        }
        if collector.is_none() {
            match Collector::new() {
                Ok((value, consumer)) => {
                    generation = generation.wrapping_add(1);
                    source = Some(Source::Local(generation));
                    rows = None;
                    collector = Some(value);
                    if let Some(consumer) = consumer
                        && consumers.send((generation, consumer)).is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    publish(&latest, &reporter, Err(error.to_string()));
                    break;
                }
            }
        }
        if let Some(collector) = collector.as_mut() {
            collector.poll();
            if Instant::now() >= next_sample {
                let current = Source::Local(generation);
                let result = collector
                    .sample(highlight)
                    .map(|publication| {
                        ConnectionUpdate::prepare(publication, current, rows.as_deref())
                    })
                    .map_err(|error| error.to_string());
                publish(&latest, &reporter, result);
                next_sample = Instant::now() + Duration::from_secs(2);
            }
        }
        match requests.recv_timeout(Duration::from_millis(50)) {
            Ok(command) => command.apply(&mut highlight, &mut rows, source, collector.as_mut()),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

impl ConnectionCommand {
    fn apply(
        self,
        highlight: &mut Highlight,
        rows: &mut Option<Arc<Rows>>,
        source: Option<Source>,
        collector: Option<&mut Collector>,
    ) {
        match self {
            Self::Configure(value) => *highlight = value,
            Self::Rows(owner, value) if source == Some(owner) => *rows = Some(value),
            Self::Failed(generation, error) if source == Some(Source::Local(generation)) => {
                if let Some(collector) = collector {
                    collector.source_failed(error);
                }
            }
            _ => {}
        }
    }
}

fn publish(latest: &Latest, reporter: &Reporter, result: Result<ConnectionUpdate, String>) {
    let old = latest.lock().ok().and_then(|mut slot| slot.replace(result));
    drop(old);
    reporter.wake();
}

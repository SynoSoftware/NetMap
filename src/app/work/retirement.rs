use super::{ConnectionUpdate, Reply, scene};
use crate::{
    app::exploration::Rows,
    connections::{ProcessId, Publication, Snapshot},
    geography::World,
    map::model::Model,
};
use std::{
    collections::HashMap,
    mem::{Discriminant, discriminant},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

type Latest = Arc<Mutex<Option<Result<ConnectionUpdate, String>>>>;

/// Plain collection payloads only. Native windows and graphics stay on the UI
/// thread. Retiring a last Arc can otherwise free hundreds of thousands of cells
/// during reply dispatch, even though their construction happened on a worker.
pub(super) enum Payload {
    Rows(Arc<Rows>),
    Model(Arc<Model>),
    World(Arc<World>),
    Publication(Publication),
    Scene(scene::Request),
    Reply(Reply),
    Latest(Latest),
    Replies(mpsc::Receiver<Reply>),
}

enum Guard {
    Rows(Arc<Rows>),
    Model(Arc<Model>),
    World(Arc<World>),
    Snapshot(Arc<Snapshot>),
    Names(Arc<HashMap<ProcessId, Box<str>>>),
    Latest(Latest),
}

impl Guard {
    fn identity(&self) -> (Discriminant<Self>, usize) {
        let pointer = match self {
            Self::Rows(value) => Arc::as_ptr(value) as usize,
            Self::Model(value) => Arc::as_ptr(value) as usize,
            Self::World(value) => Arc::as_ptr(value) as usize,
            Self::Snapshot(value) => Arc::as_ptr(value) as usize,
            Self::Names(value) => Arc::as_ptr(value) as usize,
            Self::Latest(value) => Arc::as_ptr(value) as usize,
        };
        (discriminant(self), pointer)
    }

    fn unique(&self) -> bool {
        match self {
            Self::Rows(value) => Arc::strong_count(value) == 1,
            Self::Model(value) => Arc::strong_count(value) == 1,
            Self::World(value) => Arc::strong_count(value) == 1,
            Self::Snapshot(value) => Arc::strong_count(value) == 1,
            Self::Names(value) => Arc::strong_count(value) == 1,
            Self::Latest(value) => Arc::strong_count(value) == 1,
        }
    }
}

#[derive(Default)]
struct Pending(HashMap<(Discriminant<Guard>, usize), Guard>);

impl Pending {
    fn hold(&mut self, guard: Guard) {
        // One guard per allocation: duplicate retirement must not keep two
        // strong references waiting for each other forever.
        self.0.entry(guard.identity()).or_insert(guard);
    }

    fn publication(&mut self, value: Publication) {
        self.hold(Guard::Snapshot(value.snapshot));
        self.hold(Guard::Names(value.names));
    }

    fn reply(&mut self, reply: Reply) {
        match reply {
            Reply::Scene { model, rows, .. } => {
                self.hold(Guard::Model(model));
                self.hold(Guard::Rows(rows));
            }
            Reply::Connections(Ok(value)) => {
                self.publication(value.publication);
                self.hold(Guard::Rows(value.rows));
            }
            Reply::World(Ok(world)) => self.hold(Guard::World(world)),
            _ => {}
        }
    }

    fn accept(&mut self, payload: Payload) {
        match payload {
            Payload::Rows(value) => self.hold(Guard::Rows(value)),
            Payload::Model(value) => self.hold(Guard::Model(value)),
            Payload::World(value) => self.hold(Guard::World(value)),
            Payload::Publication(value) => self.publication(value),
            Payload::Scene(value) => {
                self.hold(Guard::World(value.world));
                self.hold(Guard::Snapshot(value.snapshot));
                self.hold(Guard::Names(value.names));
            }
            Payload::Reply(value) => self.reply(value),
            Payload::Latest(value) => self.hold(Guard::Latest(value)),
            Payload::Replies(replies) => {
                for reply in replies {
                    self.reply(reply);
                }
            }
        }
    }

    fn release(&mut self) {
        // Cached renderer models can outlive the replaced Map model. Keep our
        // guard until their UI-affine owners release them; dropping a queued Arc
        // immediately would merely move the final destructor back to that UI.
        let ready: Vec<_> = self
            .0
            .extract_if(|_, guard| guard.unique())
            .map(|(_, guard)| guard)
            .collect();
        for guard in ready {
            if let Guard::Latest(latest) = guard {
                // No strong owner can clone a unique Arc. These payloads expose
                // no Weak upgrade path; workers have stopped using this slot.
                if let Ok(slot) = Arc::try_unwrap(latest)
                    && let Some(Ok(value)) =
                        slot.into_inner().unwrap_or_else(|error| error.into_inner())
                {
                    self.publication(value.publication);
                    self.hold(Guard::Rows(value.rows));
                }
            }
        }
    }
}

pub(super) fn run(receiver: mpsc::Receiver<Payload>) {
    let mut pending = Pending::default();
    let mut disconnected = false;
    loop {
        if disconnected {
            std::thread::sleep(Duration::from_millis(50));
        } else {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(payload) => pending.accept(payload),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => disconnected = true,
            }
        }
        pending.release();
        if disconnected && pending.0.is_empty() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_waits_for_cached_owners_and_duplicate_guards_do_not_pin_models() {
        let model = Arc::new(Model::default());
        let lifetime = Arc::downgrade(&model);
        let cached = Arc::clone(&model);
        let mut pending = Pending::default();
        pending.hold(Guard::Model(Arc::clone(&model)));
        pending.hold(Guard::Model(model));
        pending.release();
        assert!(lifetime.upgrade().is_some());
        drop(cached);
        pending.release();
        assert!(lifetime.upgrade().is_none());
    }
}

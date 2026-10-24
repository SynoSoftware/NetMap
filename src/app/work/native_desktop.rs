//! One scoped gate before an actual registered Desktop discovery reply.

use super::*;
use std::cell::RefCell;

thread_local! { static NEXT: RefCell<Option<DiscoveryGate>> = const { RefCell::new(None) }; }

pub(crate) struct Ready {
    pub(crate) id: u64,
    pub(crate) monitors: usize,
    pub(crate) error: Option<String>,
}

pub(crate) struct Gate {
    release: mpsc::SyncSender<()>,
    ready: mpsc::Receiver<Ready>,
    passed: Arc<AtomicBool>,
}

pub(crate) struct DiscoveryGate {
    id: u64,
    release: mpsc::Receiver<()>,
    ready: mpsc::SyncSender<Ready>,
    passed: Arc<AtomicBool>,
}

pub(crate) fn arm_next() -> io::Result<Gate> {
    let (release, gate) = mpsc::sync_channel(1);
    let (ready, result) = mpsc::sync_channel(1);
    let passed = Arc::new(AtomicBool::new(false));
    NEXT.with(|next| {
        let mut next = next.borrow_mut();
        if next.is_some() {
            return Err(io::Error::other(
                "A Desktop discovery gate is already armed",
            ));
        }
        *next = Some(DiscoveryGate {
            id: 0,
            release: gate,
            ready,
            passed: Arc::clone(&passed),
        });
        Ok(())
    })?;
    Ok(Gate {
        release,
        ready: result,
        passed,
    })
}

pub(crate) fn take_next(id: u64) -> Option<DiscoveryGate> {
    NEXT.with(|next| next.borrow_mut().take()).map(|mut gate| {
        gate.id = id;
        gate
    })
}

impl DiscoveryGate {
    pub(crate) fn before_reply(
        &self,
        result: &std::result::Result<
            (
                crate::host::desktop::Attachment,
                Vec<crate::host::desktop::Monitor>,
            ),
            String,
        >,
    ) -> bool {
        let ready = Ready {
            id: self.id,
            monitors: result.as_ref().map_or(0, |(_, monitors)| monitors.len()),
            error: result.as_ref().err().cloned(),
        };
        if self.ready.try_send(ready).is_err() {
            return false;
        }
        if self.release.recv_timeout(Duration::from_secs(10)).is_err() {
            return false;
        }
        self.passed.store(true, Ordering::Release);
        true
    }
}

impl Gate {
    pub(crate) fn ready(&self) -> Option<Ready> {
        self.ready.try_recv().ok()
    }

    pub(crate) fn release(&self) -> io::Result<()> {
        self.release
            .try_send(())
            .map_err(|error| io::Error::other(error.to_string()))
    }

    pub(crate) fn passed(&self) -> bool {
        self.passed.load(Ordering::Acquire)
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        // Remove an unused UI-side arm without disturbing a newer gate. For a
        // captured gate, dropping our sender releases its worker before finish.
        NEXT.with(|next| {
            let mut next = next.borrow_mut();
            if next
                .as_ref()
                .is_some_and(|gate| Arc::ptr_eq(&gate.passed, &self.passed))
            {
                next.take();
            }
        });
    }
}

pub(crate) fn discovery_finished(app: &Application) -> bool {
    app.work
        .borrow()
        .workers
        .iter()
        .filter(|worker| worker.thread().name() == Some("NetMap desktop discovery"))
        .all(JoinHandle::is_finished)
}

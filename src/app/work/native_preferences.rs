//! One scoped, next-save barrier for the opt-in native Settings lifetime check.

use super::native_shutdown::{self, Kind};
use std::{
    cell::RefCell,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

thread_local! { static NEXT: RefCell<Option<Gate>> = const { RefCell::new(None) }; }

pub(crate) struct Gate {
    entered: Arc<AtomicBool>,
    release: mpsc::Receiver<()>,
    exit: Option<Kind>,
}

pub(crate) struct Control {
    entered: Arc<AtomicBool>,
    release: Option<mpsc::SyncSender<()>>,
}

pub(crate) fn arm() -> io::Result<Control> {
    prepare(None)
}

pub(crate) fn arm_exit(kind: Kind) -> io::Result<Control> {
    prepare(Some(kind))
}

fn prepare(exit: Option<Kind>) -> io::Result<Control> {
    NEXT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err(io::Error::other("A next-save barrier is already armed"));
        }
        let entered = Arc::new(AtomicBool::new(false));
        let (release, receiver) = mpsc::sync_channel(1);
        *slot = Some(Gate {
            entered: Arc::clone(&entered),
            release: receiver,
            exit,
        });
        Ok(Control {
            entered,
            release: Some(release),
        })
    })
}

pub(crate) fn take_gate() -> Option<Gate> {
    NEXT.with(|slot| slot.borrow_mut().take())
}

impl Gate {
    pub(crate) fn wait(self) -> io::Result<()> {
        if let Some(kind) = self.exit {
            // This registered save worker verifies the isolated baseline with
            // the production parser before entering the pre-storage barrier.
            // Byte identity after fallback then also preserves parseability.
            let baseline = super::storage::load(&crate::app::data_directory()?)?;
            native_shutdown::record(
                kind,
                "preferences_worker_entered",
                &format!(
                    "\"baseline_bright\":{},\"gate_maximum_ms\":10000",
                    baseline.bright_seconds
                ),
            );
        }
        self.entered.store(true, Ordering::Release);
        let result = self
            .release
            .recv_timeout(Duration::from_secs(10))
            .map_err(|error| io::Error::other(format!("Native save barrier: {error}")));
        if let Some(kind) = self.exit {
            native_shutdown::record(
                kind,
                "preferences_gate_returned",
                &format!("\"released\":{}", result.is_ok()),
            );
        }
        result
    }
}

impl Control {
    pub(crate) fn entered(&self) -> bool {
        self.entered.load(Ordering::Acquire)
    }

    pub(crate) fn release(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.try_send(());
        }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        self.release();
        // If submission failed before taking the gate, restore its TLS slot.
        NEXT.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

use super::Reporter;
use std::{
    ffi::c_void,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::HANDLE,
        NetworkManagement::IpHelper::{
            CancelMibChangeNotify2, MIB_IPFORWARD_ROW2, MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE,
            NotifyIpInterfaceChange, NotifyRouteChange2,
        },
        Networking::WinSock::AF_UNSPEC,
    },
    core::Result,
};

struct Context {
    dirty: Arc<AtomicBool>,
    retired: Arc<AtomicBool>,
    active: AtomicBool,
    reporter: Reporter,
}

// Created and dropped only in the registered origin worker. Native cancel can
// wait for callbacks; it must never run on the UI or deadline-checking thread.
pub(super) struct Watch {
    handles: Vec<HANDLE>,
    context: Option<Box<Context>>,
}

impl Watch {
    pub fn new(
        dirty: Arc<AtomicBool>,
        retired: Arc<AtomicBool>,
        reporter: Reporter,
    ) -> Result<Self> {
        let mut watch = Self {
            handles: Vec::with_capacity(2),
            context: Some(Box::new(Context {
                dirty,
                retired,
                active: AtomicBool::new(true),
                reporter,
            })),
        };
        let context = (&**watch.context.as_ref().unwrap() as *const Context).cast();
        // SAFETY: Each successful registration borrows this stable allocation
        // through cancellation/drain. Initial callbacks are deliberately off.
        unsafe {
            let mut route = HANDLE::default();
            NotifyRouteChange2(AF_UNSPEC, Some(route_changed), context, false, &mut route).ok()?;
            watch.handles.push(route);
            let mut interface = HANDLE::default();
            NotifyIpInterfaceChange(
                AF_UNSPEC,
                Some(interface_changed),
                Some(context),
                false,
                &mut interface,
            )
            .ok()?;
            watch.handles.push(interface);
        }
        Ok(watch)
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let Some(context) = self.context.take() else {
            return;
        };
        context.active.store(false, Ordering::Release);
        let mut drained = true;
        for handle in self.handles.drain(..) {
            // SAFETY: Only this worker owns/cancels these registrations, outside
            // callbacks and without a lock or other resource callbacks need.
            drained &= unsafe { CancelMibChangeNotify2(handle) }.is_ok();
        }
        if !drained {
            // A failed cancellation is not proof that callbacks ended. Keep
            // their inactive context valid until process exit instead of UAF.
            let _ = Box::leak(context);
        }
    }
}

unsafe fn changed(context: *const c_void) {
    // SAFETY: Watch retains this pointer until both registrations are drained.
    let context = unsafe { &*context.cast::<Context>() };
    if context.active.load(Ordering::Acquire)
        && !context.retired.load(Ordering::Acquire)
        && !context.dirty.swap(true, Ordering::AcqRel)
    {
        // No channel send or wait: a full reply queue cannot lose the hint.
        context.reporter.wake();
    }
}

unsafe extern "system" fn route_changed(
    context: *const c_void,
    _: *const MIB_IPFORWARD_ROW2,
    _: MIB_NOTIFICATION_TYPE,
) {
    // SAFETY: Ignore OS-owned/incomplete row storage and retain only a hint.
    unsafe {
        changed(context);
    }
}

unsafe extern "system" fn interface_changed(
    context: *const c_void,
    _: *const MIB_IPINTERFACE_ROW,
    _: MIB_NOTIFICATION_TYPE,
) {
    // SAFETY: Route/interface callbacks may overlap; the shared hint is atomic.
    unsafe {
        changed(context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_reply_queue_cannot_drop_a_network_change_or_block_its_callback() {
        let (sender, replies) = std::sync::mpsc::sync_channel(8);
        for _ in 0..8 {
            sender
                .try_send(super::super::Reply::NetworkWatch(Ok(())))
                .unwrap();
        }
        let retired = Arc::new(AtomicBool::new(false));
        let dirty = Arc::new(AtomicBool::new(false));
        let context = Context {
            dirty: Arc::clone(&dirty),
            retired: Arc::clone(&retired),
            active: AtomicBool::new(true),
            reporter: Reporter {
                sender,
                cancelled: Arc::clone(&retired),
                endpoint: 0,
            },
        };
        for _ in 0..64 {
            // SAFETY: This test owns the callback context for every synchronous
            // invocation; no OS row or registration is involved.
            unsafe {
                changed((&context as *const Context).cast());
            }
        }
        assert!(dirty.swap(false, Ordering::AcqRel));
        assert_eq!(replies.try_iter().count(), 8);
        retired.store(true, Ordering::Release);
        // SAFETY: The same live test allocation remains available after retire.
        unsafe {
            changed((&context as *const Context).cast());
        }
        assert!(!dirty.load(Ordering::Acquire));
    }
}

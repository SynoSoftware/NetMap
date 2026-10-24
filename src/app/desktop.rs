#[derive(Default)]
pub(super) struct Discovery {
    requested: u64,
    running: Option<u64>,
}

pub(super) enum Completion {
    Ignore,
    Publish,
    Refresh(u64),
}

impl Discovery {
    pub fn request(&mut self) -> Option<u64> {
        self.requested = self.requested.wrapping_add(1);
        if self.running.is_some() {
            None
        } else {
            self.running = Some(self.requested);
            Some(self.requested)
        }
    }

    pub fn complete(&mut self, id: u64) -> Completion {
        if self.running != Some(id) {
            return Completion::Ignore;
        }
        if id != self.requested {
            self.running = Some(self.requested);
            Completion::Refresh(self.requested)
        } else {
            self.running = None;
            Completion::Publish
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_changes_during_discovery_publish_only_the_latest_topology() {
        let mut discovery = Discovery::default();
        let first = discovery.request().unwrap();
        assert!(discovery.request().is_none());
        assert!(discovery.request().is_none());
        let Completion::Refresh(latest) = discovery.complete(first) else {
            panic!("An obsolete discovery must refresh, not publish");
        };
        assert!(matches!(discovery.complete(first), Completion::Ignore));
        assert!(matches!(discovery.complete(latest), Completion::Publish));
        let next = discovery.request().unwrap();
        assert_ne!(next, latest);
        assert!(matches!(discovery.complete(latest), Completion::Ignore));
        assert!(matches!(discovery.complete(next), Completion::Publish));
    }
}

mod collector;
pub(crate) mod etw;
mod model;
pub(crate) mod pipe;
mod processes;
mod tables;

pub(crate) use collector::{Collector, Publication};
pub use model::*;

#[cfg(test)]
pub(crate) fn native_tcp_owners() -> Vec<(u32, Endpoint, Endpoint)> {
    tables::read(std::time::Duration::default)
        .rows
        .into_iter()
        .map(|row| (row.pid, row.local, row.remote))
        .collect()
}

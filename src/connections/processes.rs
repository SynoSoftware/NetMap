use std::{
    collections::{HashMap, HashSet},
    io,
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::Arc,
    time::Duration,
};
use windows::{
    Wdk::System::SystemInformation::{NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS},
    Win32::{
        Foundation::{HANDLE, UNICODE_STRING},
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
    core::PWSTR,
};

use super::{
    Observation, ProcessId, Window,
    etw::{Clock, ProcessFact, ProcessKind},
};

pub(super) const MAX_NAME_BYTES: usize = 1024;
const NAME_QUERIES_PER_SAMPLE: usize = 32;

pub(super) fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= MAX_NAME_BYTES
        && !name
            .chars()
            .any(|value| value.is_control() || matches!(value, '\\' | '/' | ':'))
}

fn process_name(pid: u32) -> Option<Box<str>> {
    // SAFETY: A limited-information handle is opened only for the candidate PID.
    // Its instance identity is validated by censuses on both sides of this query.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    // SAFETY: The successful open transfers its unique close obligation here.
    let handle = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    let mut path = vec![0_u16; 32768];
    let mut length = path.len() as u32;
    // SAFETY: Handle and complete initialized output remain live through the
    // synchronous query; length is in UTF-16 units, excluding the terminator.
    unsafe {
        QueryFullProcessImageNameW(
            HANDLE(handle.as_raw_handle()),
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    let path = String::from_utf16(path.get(..length as usize)?).ok()?;
    let name = path.rsplit(['\\', '/']).next()?;
    valid_name(name).then(|| Box::<str>::from(name))
}

// SDK winternl.h (10.0.28000): windows-rs does not yet expose class 252 or
// SYSTEM_BASICPROCESS_INFORMATION. SequenceNumber detects reuse without
// opening individual processes, including those whose metadata is restricted.
#[repr(C)]
#[derive(Clone, Copy)]
struct BasicProcess {
    next: u32,
    pid: HANDLE,
    parent: HANDLE,
    sequence: u64,
    name: UNICODE_STRING,
}

pub(super) struct Census {
    pub window: Window,
    pub sequences: HashMap<u32, u64>,
}

impl Census {
    pub fn read(clock: Clock) -> io::Result<Self> {
        let begin = clock.now()?;
        let mut buffer = vec![0_u64; 8192];
        for _ in 0..4 {
            let capacity = size_of_val(buffer.as_slice());
            let mut returned = 0;
            // SAFETY: Initialized, aligned storage is exclusively borrowed for
            // this synchronous call. HANDLE fields are numeric IDs, not owned
            // handles. ImageName pointers are neither followed nor retained.
            let status = unsafe {
                NtQuerySystemInformation(
                    SYSTEM_INFORMATION_CLASS(252),
                    buffer.as_mut_ptr().cast(),
                    capacity as u32,
                    &mut returned,
                )
            };
            if status.is_ok() {
                let length = returned as usize;
                if length > capacity || length < size_of::<BasicProcess>() {
                    break;
                }
                let mut offset = 0;
                let mut sequences = HashMap::new();
                loop {
                    if offset > length - size_of::<BasicProcess>() {
                        return Err(io::Error::other("Invalid process census"));
                    }
                    // SAFETY: The complete record lies inside initialized native
                    // output. Unaligned reading also handles a malformed offset
                    // without creating an invalid Rust reference.
                    let process = unsafe {
                        buffer
                            .as_ptr()
                            .cast::<u8>()
                            .add(offset)
                            .cast::<BasicProcess>()
                            .read_unaligned()
                    };
                    let pid = u32::try_from(process.pid.0 as usize)
                        .map_err(|_| io::Error::other("Invalid process ID"))?;
                    sequences.insert(pid, process.sequence);
                    if process.next == 0 {
                        return Ok(Self {
                            window: Window {
                                begin,
                                end: clock.now()?,
                            },
                            sequences,
                        });
                    }
                    if (process.next as usize) < size_of::<BasicProcess>() {
                        break;
                    }
                    offset = offset
                        .checked_add(process.next as usize)
                        .ok_or_else(|| io::Error::other("Invalid process offset"))?;
                }
                break;
            }
            // STATUS_INFO_LENGTH_MISMATCH. Unsupported classes and other
            // failures remain coverage limitations rather than guessed identity.
            if status.0 as u32 != 0xc0000004 {
                return Err(io::Error::other(format!(
                    "Process census: NTSTATUS {:08x}",
                    status.0 as u32
                )));
            }
            let required = (returned as usize).max(capacity.saturating_mul(2));
            if required > 16 * 1024 * 1024 {
                break;
            }
            buffer.resize(required.div_ceil(size_of::<u64>()), 0);
        }
        Err(io::Error::other(
            "Process census exceeded its bounded capture",
        ))
    }

    pub fn stable(&self, other: &Self, pid: u32) -> Option<u64> {
        let sequence = *self.sequences.get(&pid)?;
        (other.sequences.get(&pid) == Some(&sequence)).then_some(sequence)
    }
}

#[derive(Clone, Copy)]
struct Seen {
    first: Window,
    last: Window,
}

#[derive(Clone, Copy)]
struct Trace {
    epoch: u64,
    first: Duration,
    born: bool,
    key: Option<u64>,
    end: Option<Duration>,
    trusted: bool,
}

struct Episode {
    id: ProcessId,
    sequence: Option<u64>,
    seen: Option<Seen>,
    missing: bool,
    trace: Option<Trace>,
    used: bool,
}

impl Episode {
    fn new(pid: u32, generation: &mut u64) -> Self {
        *generation += 1;
        Self {
            id: ProcessId {
                pid,
                generation: *generation,
            },
            sequence: None,
            seen: None,
            missing: false,
            trace: None,
            used: false,
        }
    }

    fn contains(&self, at: Duration) -> bool {
        if self
            .seen
            .is_some_and(|seen| at > seen.first.end && at < seen.last.begin)
        {
            return true;
        }
        self.trace.is_some_and(|trace| {
            trace.trusted
                && at > trace.first
                && trace.end.is_none_or(|end| at < end)
                && (!self.missing || trace.end.is_some())
        })
    }
}

#[derive(Default)]
pub(super) struct Processes {
    episodes: HashMap<u32, Vec<Episode>>,
    ended: Vec<ProcessFact>,
    next: u64,
    rundown: bool,
    epoch: u64,
    since: Duration,
    names: HashMap<ProcessId, Option<Box<str>>>,
}

impl Processes {
    fn current_sequence(&self, process: ProcessId) -> Option<u64> {
        self.episodes
            .get(&process.pid)?
            .iter()
            .find(|episode| episode.id == process && !episode.missing)?
            .sequence
    }

    pub fn names(
        &mut self,
        observations: &[Observation],
        before: Option<&Census>,
        clock: Clock,
    ) -> Arc<HashMap<ProcessId, Box<str>>> {
        let interested: HashSet<_> = observations.iter().map(|row| row.tuple.process).collect();
        self.names.retain(|process, _| interested.contains(process));
        if let Some(before) = before {
            let queried: Vec<_> = interested
                .iter()
                .filter_map(|&process| {
                    if self.names.contains_key(&process) {
                        return None;
                    }
                    let sequence = self.current_sequence(process)?;
                    (before.sequences.get(&process.pid) == Some(&sequence))
                        .then_some((process, sequence))
                })
                .take(NAME_QUERIES_PER_SAMPLE)
                .map(|(process, sequence)| (process, sequence, process_name(process.pid)))
                .collect();
            if !queried.is_empty()
                && let Ok(after) = Census::read(clock)
            {
                self.accept_names(before, &after, queried);
            }
        }
        Arc::new(
            self.names
                .iter()
                .filter_map(|(&process, name)| name.as_ref().map(|name| (process, name.clone())))
                .collect(),
        )
    }

    fn accept_names(
        &mut self,
        before: &Census,
        after: &Census,
        queried: Vec<(ProcessId, u64, Option<Box<str>>)>,
    ) {
        for (process, sequence, name) in queried {
            // A PID can change owner between OpenProcess and the native query.
            // Only this published episode's unchanged native sequence accepts
            // either a name or a cached unavailable result. Unknown stays unknown.
            if before.stable(after, process.pid) == Some(sequence)
                && self.current_sequence(process) == Some(sequence)
            {
                self.names.insert(process, name);
            }
        }
    }

    pub fn observe(&mut self, census: &Census) {
        self.ended
            .retain(|fact| census.window.end.saturating_sub(fact.at) < Duration::from_secs(120));
        for episodes in self.episodes.values_mut() {
            for episode in episodes {
                if let Some(sequence) = episode.sequence {
                    episode.missing = census.sequences.get(&episode.id.pid) != Some(&sequence);
                }
            }
        }
        for (&pid, &sequence) in &census.sequences {
            let episodes = self.episodes.entry(pid).or_default();
            let index = episodes
                .iter()
                .position(|known| known.sequence == Some(sequence))
                .unwrap_or_else(|| {
                    let mut episode = Episode::new(pid, &mut self.next);
                    episode.sequence = Some(sequence);
                    episodes.push(episode);
                    episodes.len() - 1
                });
            let episode = &mut episodes[index];
            let seen = episode.seen.get_or_insert(Seen {
                first: census.window,
                last: census.window,
            });
            seen.last = census.window;
            episode.missing = false;
        }
        // Retained intervals let late delivery resolve to the original process,
        // never whichever instance happens to own its PID now.
        self.episodes.retain(|_, episodes| {
            episodes.retain(|episode| {
                let last = episode
                    .trace
                    .and_then(|trace| trace.end)
                    .or_else(|| episode.seen.map(|seen| seen.last.end))
                    .or_else(|| episode.trace.map(|trace| trace.first));
                last.is_none_or(|last| {
                    census.window.end.saturating_sub(last) < Duration::from_secs(120)
                })
            });
            !episodes.is_empty()
        });
    }

    pub fn event(&mut self, fact: ProcessFact) {
        if fact.at <= self.since {
            return;
        }
        let episodes = self.episodes.entry(fact.pid).or_default();
        if fact.kind == ProcessKind::Existing {
            self.rundown = true;
        }
        if fact.kind == ProcessKind::Ended {
            // ProcessTrace can deliver an exit before its start. Keep the
            // boundary independently until the matching lifetime is known.
            if !self.ended.contains(&fact) {
                self.ended.push(fact);
            }
            let candidates: Vec<_> = episodes
                .iter()
                .enumerate()
                .filter(|(_, episode)| {
                    episode.trace.is_some_and(|trace| {
                        trace.epoch == self.epoch
                            && trace.key == fact.key
                            && trace.first <= fact.at
                            && trace.end.is_none_or(|end| fact.at <= end)
                    })
                })
                .map(|(index, _)| index)
                .collect();
            if candidates.len() == 1 {
                episodes[candidates[0]].trace.as_mut().unwrap().end = Some(fact.at);
            }
            return;
        }
        if episodes.iter().any(|episode| {
            episode
                .trace
                .is_some_and(|trace| trace.first == fact.at && trace.key == fact.key)
        }) {
            return;
        }
        if fact.kind == ProcessKind::Started {
            for episode in episodes.iter_mut() {
                if let Some(trace) = &mut episode.trace
                    && trace.first < fact.at
                    && trace.end.is_none()
                {
                    trace.end = Some(fact.at);
                }
            }
        } else if episodes.iter().any(|episode| {
            episode.trace.is_some_and(|trace| {
                trace.key == fact.key
                    && trace.first <= fact.at
                    && trace.end.is_none_or(|end| fact.at < end)
            })
        }) {
            return;
        }
        let end = episodes
            .iter()
            .filter_map(|episode| {
                episode
                    .trace
                    .filter(|trace| trace.born && trace.first > fact.at)
                    .map(|trace| trace.first)
            })
            .chain(self.ended.iter().filter_map(|ended| {
                (ended.pid == fact.pid && ended.key == fact.key && ended.at >= fact.at)
                    .then_some(ended.at)
            }))
            .min();
        let mut episode = Episode::new(fact.pid, &mut self.next);
        episode.trace = Some(Trace {
            epoch: self.epoch,
            first: fact.at,
            born: fact.kind == ProcessKind::Started,
            key: fact.key,
            end,
            trusted: true,
        });
        episodes.push(episode);
    }

    /// Bind complementary evidence before publishing rows/events. Never merge
    /// two identities that have both already escaped into observation snapshots.
    pub fn link(&mut self, now: Duration) {
        for episodes in self.episodes.values_mut() {
            let mut index = 0;
            while index < episodes.len() {
                let Some(mut trace) = episodes[index]
                    .trace
                    .filter(|_| episodes[index].sequence.is_none())
                else {
                    index += 1;
                    continue;
                };
                if trace.epoch != self.epoch {
                    index += 1;
                    continue;
                }
                let end = trace.end.unwrap_or(now);
                let candidates: Vec<_> = episodes
                    .iter()
                    .enumerate()
                    .filter(|(_, episode)| {
                        episode.sequence.is_some()
                            && episode.trace.is_none()
                            && episode.seen.is_some_and(|seen| {
                                (seen.first.begin > trace.first && seen.last.end < end)
                                    || (trace.born
                                        && seen.first.end >= trace.first
                                        && seen.last.begin > trace.first
                                        && seen.last.end < end)
                                    || (!trace.born
                                        && seen.first.end < trace.first
                                        && seen.last.begin > trace.first
                                        && seen.last.end < end)
                            })
                    })
                    .map(|(index, _)| index)
                    .collect();
                if candidates.len() != 1 {
                    trace.trusted = trace.epoch == self.epoch
                        && candidates.is_empty()
                        && (trace.born
                            || !episodes.iter().any(|episode| episode.sequence.is_some()));
                    episodes[index].trace = Some(trace);
                    index += 1;
                    continue;
                }
                let candidate = candidates[0];
                if episodes[index].used && episodes[candidate].used {
                    episodes[index].trace.as_mut().unwrap().trusted = false;
                    index += 1;
                    continue;
                }
                let (keep, remove) = if episodes[index].used {
                    (index, candidate)
                } else {
                    (candidate, index)
                };
                let removed = episodes.remove(remove);
                let keep = if keep > remove { keep - 1 } else { keep };
                let retained = &mut episodes[keep];
                retained.sequence = retained.sequence.or(removed.sequence);
                retained.seen = retained.seen.or(removed.seen);
                retained.missing |= removed.missing;
                trace.trusted = trace.epoch == self.epoch;
                retained.trace = Some(trace);
                index = 0;
            }
        }
    }

    pub fn sequence(&mut self, pid: u32, sequence: u64) -> Option<ProcessId> {
        let episode = self
            .episodes
            .get_mut(&pid)?
            .iter_mut()
            .find(|episode| episode.sequence == Some(sequence))?;
        episode.used = true;
        Some(episode.id)
    }

    pub fn resolve(&mut self, pid: u32, at: Duration) -> Option<ProcessId> {
        let episodes = self.episodes.get_mut(&pid)?;
        let mut candidates = episodes
            .iter()
            .enumerate()
            .filter(|(_, episode)| episode.contains(at))
            .map(|(index, _)| index);
        let index = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        episodes[index].used = true;
        Some(episodes[index].id)
    }

    pub fn has_rundown(&self) -> bool {
        self.rundown
    }

    pub fn invalidate_trace(&mut self, at: Duration) {
        self.epoch += 1;
        self.since = at;
        self.ended.clear();
        for episodes in self.episodes.values_mut() {
            for episode in episodes {
                if let Some(trace) = &mut episode.trace {
                    trace.trusted = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;

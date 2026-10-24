use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

const UDP_IDLE: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservationId(pub u64);

/// An application-owned process episode, never a PID treated as durable identity.
/// Native collection allocates a new generation when instance continuity is lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProcessId {
    pub pid: u32,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Endpoint {
    address: IpAddr,
    port: u16,
    scope: u32,
}

impl Endpoint {
    pub fn new(socket: SocketAddr) -> Self {
        let (address, scope) = match socket {
            SocketAddr::V4(address) => (IpAddr::V4(*address.ip()), 0),
            SocketAddr::V6(address) => match address.ip().to_ipv4_mapped() {
                Some(ip) => (IpAddr::V4(ip), 0),
                None => (IpAddr::V6(*address.ip()), address.scope_id()),
            },
        };
        Self {
            address,
            port: socket.port(),
            scope,
        }
    }

    pub fn address(self) -> IpAddr {
        self.address
    }
    pub fn port(self) -> u16 {
        self.port
    }
    pub fn scope(self) -> u32 {
        self.scope
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tuple {
    pub process: ProcessId,
    pub local: Endpoint,
    pub remote: Endpoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    DeleteTcb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Life {
    Tcp(TcpState),
    EndedTcp,
    Udp { last_seen: Duration },
    ExpiredUdp { last_seen: Duration },
}

impl Life {
    fn ended(self) -> bool {
        matches!(self, Self::EndedTcp | Self::ExpiredUdp { .. })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bytes {
    pub received: u64,
    pub sent: u64,
}

impl Bytes {
    fn add(&mut self, other: Self) {
        self.received += other.received;
        self.sent += other.sent;
    }

    pub fn active(self) -> bool {
        self.received != 0 || self.sent != 0
    }

    pub fn rates(self, elapsed: Duration) -> Option<Rates> {
        (!elapsed.is_zero()).then(|| Rates {
            received: self.received as f64 / elapsed.as_secs_f64(),
            sent: self.sent as f64 / elapsed.as_secs_f64(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rates {
    pub received: f64,
    pub sent: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub begin: Duration,
    pub end: Duration,
}

impl Window {
    fn point(at: Duration) -> Self {
        Self { begin: at, end: at }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    TcpOpened {
        tuple: Tuple,
        at: Duration,
    },
    TcpClosed {
        tuple: Tuple,
        at: Duration,
    },
    Transfer {
        tuple: Tuple,
        protocol: Protocol,
        at: Duration,
        bytes: Bytes,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct TcpRow {
    pub tuple: Tuple,
    pub state: TcpState,
}

/// Encloses all native table reads contributing to this pass. A partial pass
/// supplies positive rows but cannot establish absence in the unread tables.
pub struct Table {
    pub capture: Window,
    pub complete: bool,
    pub rows: Vec<TcpRow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    WarmingUp,
    Complete,
    Restricted,
    Lost { events: u64, buffers: u64 },
}

#[derive(Clone, Copy, Debug)]
pub struct Highlight {
    pub bright: Duration,
    pub fade: Duration,
}

impl Highlight {
    pub fn duration(self) -> Duration {
        self.bright.saturating_add(self.fade)
    }

    pub fn opacity(self, introduced: Duration, now: Duration, static_marks: bool) -> f32 {
        let age = now.saturating_sub(introduced);
        if age >= self.duration() {
            return 0.0;
        }
        if static_marks || age < self.bright {
            return 1.0;
        }
        1.0 - (age - self.bright).as_secs_f32() / self.fade.as_secs_f32()
    }

    pub fn deadline(
        self,
        introduced: Duration,
        now: Duration,
        static_marks: bool,
    ) -> Option<Duration> {
        let bright = introduced.saturating_add(self.bright);
        let end = introduced.saturating_add(self.duration());
        if !static_marks && now < bright {
            Some(bright)
        } else if now < end {
            Some(end)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub id: ObservationId,
    pub tuple: Tuple,
    pub life: Life,
    pub introduced: Duration,
    pub bytes: Bytes,
    pub rates: Option<Rates>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub counts: Counts,
    pub observations: Arc<[Observation]>,
    pub interval: Window,
    pub coverage: Coverage,
    pub bytes: Bytes,
    pub unattributed: Bytes,
    pub total_bytes: Bytes,
    pub rates: Option<Rates>,
    pub ambiguous_events: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub tcp: usize,
    pub ended_tcp: usize,
    pub udp: usize,
    pub inactive_udp: usize,
    pub transferring: usize,
}

#[derive(Clone, Copy, Debug)]
enum Birth {
    Opened(Duration),
    Observed {
        window: Window,
        after: Option<Window>,
    },
}

impl Birth {
    fn observed(window: Window) -> Self {
        Self::Observed {
            window,
            after: None,
        }
    }

    fn upper(self) -> Duration {
        match self {
            Self::Opened(at) => at,
            Self::Observed { window, .. } => window.end,
        }
    }

    fn lower(self) -> Option<Duration> {
        match self {
            Self::Opened(at) => Some(at),
            Self::Observed { after, .. } => after.map(|window| window.begin),
        }
    }

    fn successor_to(self, at: Duration) -> Option<Duration> {
        match self {
            Self::Opened(birth) if birth > at => Some(birth),
            Self::Observed {
                after: Some(window),
                ..
            } if at <= window.begin => Some(window.end),
            _ => None,
        }
    }
}

enum State {
    Tcp(TcpState),
    EndedTcp,
    Udp,
}

struct Record {
    id: ObservationId,
    birth: Birth,
    end: Option<Window>,
    evidence: Duration,
    state: State,
    introduced: Option<Duration>,
    pending: Bytes,
    uncertain: bool,
    gap: bool,
}

impl Record {
    fn contains(&self, at: Duration) -> bool {
        !self.gap && at > self.birth.upper() && self.end.is_none_or(|end| at <= end.begin)
    }

    fn can_close(&self, at: Duration) -> bool {
        !self.gap
            && self.birth.lower().is_none_or(|birth| at > birth)
            && at >= self.evidence
            && self
                .end
                .is_none_or(|end| at <= end.end && (end.begin != end.end || at == end.end))
    }

    fn overlaps(&self, window: Window) -> bool {
        self.birth.lower().is_none_or(|birth| birth <= window.end)
            && self.end.is_none_or(|end| end.end >= window.begin)
    }

    fn end_tcp(&mut self, window: Window) {
        self.end = Some(window);
        self.state = State::EndedTcp;
    }

    fn life(&self, now: Duration) -> Life {
        match self.state {
            State::Tcp(state) => Life::Tcp(state),
            State::EndedTcp => Life::EndedTcp,
            State::Udp if self.end.is_some() || now >= self.evidence.saturating_add(UDP_IDLE) => {
                Life::ExpiredUdp {
                    last_seen: self.evidence,
                }
            }
            State::Udp => Life::Udp {
                last_seen: self.evidence,
            },
        }
    }
}

#[derive(Default)]
struct History {
    records: Vec<Record>,
    // Keep retired source bounds only while this tuple still has observations.
    retired_through: Option<Duration>,
}

/// Owns observation identity, measured-byte intervals and publication retention.
/// Source timestamps and publication times share the collector's monotonic epoch.
pub struct Model {
    histories: HashMap<(Protocol, Tuple), History>,
    endpoints: HashMap<(Protocol, Endpoint, Endpoint), HashSet<ProcessId>>,
    table: Option<(Window, HashSet<(Protocol, Tuple)>)>,
    next_id: u64,
    published: Duration,
    pending: Bytes,
    unattributed: Bytes,
    total: Bytes,
    ambiguous: u64,
}

impl Model {
    pub fn new(at: Duration) -> Self {
        Self {
            histories: HashMap::new(),
            endpoints: HashMap::new(),
            table: None,
            next_id: 0,
            published: at,
            pending: Bytes::default(),
            unattributed: Bytes::default(),
            total: Bytes::default(),
            ambiguous: 0,
        }
    }

    fn record(&mut self, key: (Protocol, Tuple), mut birth: Birth, state: State) -> Record {
        self.endpoints
            .entry((key.0, key.1.local, key.1.remote))
            .or_default()
            .insert(key.1.process);
        let absence = self
            .table
            .as_ref()
            .filter(|(_, present)| key.0 == Protocol::Tcp && !present.contains(&key))
            .map(|(capture, _)| *capture);
        let evidence = match &mut birth {
            Birth::Opened(at) => *at,
            Birth::Observed { window, after } => {
                *after = absence.filter(|capture| capture.end <= window.begin);
                window.begin
            }
        };
        self.next_id += 1;
        let mut record = Record {
            id: ObservationId(self.next_id),
            birth,
            end: None,
            evidence,
            state,
            introduced: None,
            pending: Bytes::default(),
            uncertain: false,
            gap: false,
        };
        if let Some(capture) = absence.filter(|capture| capture.begin > birth.upper()) {
            record.end_tcp(Window {
                begin: evidence,
                end: capture.end,
            });
        }
        record
    }

    fn record_ambiguity(&mut self, affected: Option<(Protocol, Tuple)>, bytes: Bytes) {
        self.unattributed.add(bytes);
        self.ambiguous += 1;
        if let Some(history) = affected.and_then(|key| self.histories.get_mut(&key)) {
            for record in &mut history.records {
                record.uncertain = true;
            }
        }
    }

    pub fn event(&mut self, event: Event) {
        match event {
            Event::TcpOpened { tuple, at } => self.open(tuple, at),
            Event::TcpClosed { tuple, at } => self.close(tuple, at),
            Event::Transfer {
                tuple,
                protocol,
                at,
                bytes,
            } => self.transfer(tuple, protocol, at, bytes),
        }
    }

    /// Native evidence without an unambiguous process or endpoint identity
    /// still contributes exactly once to the measured byte totals.
    pub fn unattributed(
        &mut self,
        protocol: Protocol,
        local: Endpoint,
        remote: Endpoint,
        at: Duration,
        bytes: Bytes,
    ) {
        self.pending.add(bytes);
        self.total.add(bytes);
        self.record_ambiguity(None, bytes);
        if let Some(processes) = self.endpoints.get(&(protocol, local, remote)) {
            for &process in processes {
                let tuple = Tuple {
                    process,
                    local,
                    remote,
                };
                let Some(history) = self.histories.get_mut(&(protocol, tuple)) else {
                    continue;
                };
                for record in &mut history.records {
                    if record.overlaps(Window::point(at)) {
                        record.uncertain = true;
                    }
                }
            }
        }
    }

    /// A System-context event does not establish socket ownership. Resolve it
    /// only inside one known TCP lifetime, including retained ended lifetimes.
    /// The index narrows the search; histories remain the authority for time,
    /// reuse and loss. A boundary or a competing possible lifetime is ambiguous.
    pub(super) fn tcp_owner(
        &self,
        local: Endpoint,
        remote: Endpoint,
        at: Duration,
    ) -> Option<ProcessId> {
        let processes = self.endpoints.get(&(Protocol::Tcp, local, remote))?;
        let mut owner = None;
        for &process in processes {
            let tuple = Tuple {
                process,
                local,
                remote,
            };
            let history = self.histories.get(&(Protocol::Tcp, tuple))?;
            if history.retired_through.is_some_and(|end| at <= end) {
                return None;
            }
            for record in &history.records {
                if record.overlaps(Window::point(at)) {
                    if !record.contains(at) || owner.is_some() {
                        return None;
                    }
                    owner = Some(process);
                }
            }
        }
        owner
    }

    /// Lost lifecycle evidence can conceal a same-tuple TCP replacement.
    /// Preserve published identities without attaching later traffic to them;
    /// new, independently established incarnations have their own records.
    pub fn gap(&mut self, through: Duration) {
        for ((protocol, _), history) in &mut self.histories {
            if *protocol == Protocol::Tcp {
                for record in &mut history.records {
                    if record.birth.lower().is_none_or(|birth| birth <= through) {
                        record.gap = true;
                    }
                }
            }
        }
    }

    fn open(&mut self, tuple: Tuple, at: Duration) {
        let key = (Protocol::Tcp, tuple);
        let history = self.histories.entry(key).or_default();
        if history.retired_through.is_some_and(|end| at <= end) {
            return;
        }
        let records = &mut history.records;
        if records
            .iter()
            .any(|record| matches!(record.birth, Birth::Opened(birth) if birth == at))
        {
            return;
        }
        // A delayed unique opening may explain a provisional startup row. It
        // must precede that capture and all other known lifetimes at this tuple.
        let provisional: Vec<_> = records
            .iter()
            .enumerate()
            .filter_map(|(index, record)| match record.birth {
                Birth::Observed { window, after }
                    if at <= window.begin
                        && after.is_none_or(|absence| at > absence.begin)
                        && record.end.is_none_or(|end| at < end.begin) =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect();
        let conflict = records.iter().any(|record| {
            matches!(record.birth, Birth::Opened(birth) if birth <= at)
                && record.end.is_none_or(|end| at <= end.end)
        });
        if provisional.len() == 1 && !conflict {
            records[provisional[0]].birth = Birth::Opened(at);
            return;
        }
        if !provisional.is_empty() {
            self.record_ambiguity(Some(key), Bytes::default());
            return;
        }
        for record in records.iter_mut() {
            if record.end.is_none() && record.birth.upper() < at {
                record.end_tcp(Window {
                    begin: record.evidence,
                    end: at,
                });
            }
        }
        let successor = records
            .iter()
            .filter_map(|record| record.birth.successor_to(at))
            .min();
        let mut record = self.record(key, Birth::Opened(at), State::Tcp(TcpState::Established));
        if let Some(end) = successor {
            if record.end.is_none_or(|known| end < known.end) {
                record.end_tcp(Window { begin: at, end });
            }
            record.uncertain = true;
        }
        self.histories.entry(key).or_default().records.push(record);
    }

    fn close(&mut self, tuple: Tuple, at: Duration) {
        let key = (Protocol::Tcp, tuple);
        if !self.histories.contains_key(&key) {
            let mut record = self.record(key, Birth::observed(Window::point(at)), State::EndedTcp);
            record.end = Some(Window::point(at));
            self.histories.entry(key).or_default().records.push(record);
            return;
        }
        let history = self.histories.get_mut(&key).unwrap();
        if history.retired_through.is_some_and(|end| at <= end) {
            return;
        }
        let records = &mut history.records;
        let boundary = records
            .iter()
            .any(|record| record.birth.lower() == Some(at));
        let matching: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.can_close(at))
            .map(|(index, _)| index)
            .collect();
        if matching.is_empty()
            && records
                .iter()
                .any(|record| record.gap && record.overlaps(Window::point(at)))
        {
            self.record_ambiguity(None, Bytes::default());
            return;
        }
        if !boundary && matching.len() == 1 {
            let record = &mut records[matching[0]];
            record.end_tcp(Window::point(at));
            return;
        }
        if !boundary
            && matching.is_empty()
            && records
                .iter()
                .all(|record| record.birth.lower().is_some_and(|birth| at < birth))
        {
            let mut record = self.record(key, Birth::observed(Window::point(at)), State::EndedTcp);
            record.end = Some(Window::point(at));
            self.histories.entry(key).or_default().records.push(record);
            return;
        }
        self.record_ambiguity(Some(key), Bytes::default());
    }

    fn transfer(&mut self, tuple: Tuple, protocol: Protocol, at: Duration, bytes: Bytes) {
        if !bytes.active() {
            return;
        }
        self.pending.add(bytes);
        self.total.add(bytes);
        let key = (protocol, tuple);
        if protocol == Protocol::Udp {
            self.udp(tuple, at, bytes);
            return;
        }
        if !self.histories.contains_key(&key) {
            // Existing traffic can precede the first endpoint pass. Its opening
            // remains unknown; the first event itself is direct activity evidence.
            let mut record = self.record(
                key,
                Birth::observed(Window::point(at)),
                State::Tcp(TcpState::Established),
            );
            record.pending = bytes;
            self.histories.entry(key).or_default().records.push(record);
            return;
        }
        let history = self.histories.get_mut(&key).unwrap();
        if history.retired_through.is_some_and(|end| at <= end) {
            self.record_ambiguity(None, bytes);
            return;
        }
        let records = &mut history.records;
        let boundary = records
            .iter()
            .any(|record| record.birth.lower() == Some(at));
        let matching: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.contains(at))
            .map(|(index, _)| index)
            .collect();
        if matching.is_empty()
            && records
                .iter()
                .any(|record| record.gap && record.overlaps(Window::point(at)))
        {
            self.record_ambiguity(None, bytes);
            return;
        }
        if !boundary && matching.len() == 1 {
            let record = &mut records[matching[0]];
            record.pending.add(bytes);
            record.evidence = record.evidence.max(at);
        } else {
            self.record_ambiguity(Some(key), bytes);
        }
    }

    fn udp(&mut self, tuple: Tuple, at: Duration, bytes: Bytes) {
        let key = (Protocol::Udp, tuple);
        let history = self.histories.entry(key).or_default();
        if history.retired_through.is_some_and(|end| at <= end) {
            self.record_ambiguity(None, bytes);
            return;
        }
        let records = &mut history.records;
        let matching: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(_, record)| {
                at >= record.birth.upper()
                    && at < record.evidence.saturating_add(UDP_IDLE)
                    && record.end.is_none_or(|end| at < end.end)
            })
            .map(|(index, _)| index)
            .collect();
        if matching.len() == 1 {
            let record = &mut records[matching[0]];
            record.pending.add(bytes);
            record.evidence = record.evidence.max(at);
            if record
                .end
                .is_some_and(|end| record.evidence.saturating_add(UDP_IDLE) > end.end)
            {
                // Late evidence bridges what previously looked like inactivity.
                // Keep published identities, but leave the overlap unattributed.
                record.end = None;
                self.record_ambiguity(Some(key), Bytes::default());
            }
            return;
        }
        if matching.len() > 1 || records.iter().any(|record| at < record.birth.upper()) {
            self.record_ambiguity(Some(key), bytes);
            return;
        }
        for record in records.iter_mut().filter(|record| record.end.is_none()) {
            record.end = Some(Window::point(at));
        }
        let mut record = self.record(key, Birth::observed(Window::point(at)), State::Udp);
        record.pending = bytes;
        self.histories.entry(key).or_default().records.push(record);
    }

    pub fn reconcile(&mut self, table: Table) {
        let mut present = HashSet::new();
        for row in table.rows {
            let key = (Protocol::Tcp, row.tuple);
            present.insert(key);
            let records = &mut self.histories.entry(key).or_default().records;
            let matching: Vec<_> = records
                .iter()
                .enumerate()
                .filter(|(_, record)| record.overlaps(table.capture))
                .map(|(index, _)| index)
                .collect();
            if matching.len() == 1 {
                let record = &mut records[matching[0]];
                if !matches!(record.state, State::EndedTcp) {
                    record.evidence = record.evidence.max(table.capture.begin);
                    record.state = State::Tcp(row.state);
                }
            } else if matching.is_empty() {
                let record =
                    self.record(key, Birth::observed(table.capture), State::Tcp(row.state));
                self.histories.entry(key).or_default().records.push(record);
            } else {
                self.record_ambiguity(Some(key), Bytes::default());
            }
        }
        if !table.complete {
            return;
        }
        for (&key, history) in &mut self.histories {
            if key.0 != Protocol::Tcp || present.contains(&key) {
                continue;
            }
            for record in history
                .records
                .iter_mut()
                .filter(|record| !matches!(record.state, State::EndedTcp))
            {
                if record.birth.upper() < table.capture.begin
                    && record.evidence < table.capture.begin
                {
                    record.end_tcp(Window {
                        begin: record.evidence,
                        end: table.capture.end,
                    });
                }
            }
        }
        if self
            .table
            .as_ref()
            .is_none_or(|(capture, _)| table.capture.end >= capture.end)
        {
            self.table = Some((table.capture, present));
        }
    }

    pub fn publish(&mut self, now: Duration, highlight: Highlight, coverage: Coverage) -> Snapshot {
        let interval = Window {
            begin: self.published,
            end: now,
        };
        let elapsed = now.saturating_sub(self.published);
        let complete = coverage == Coverage::Complete && !elapsed.is_zero();
        let mut observations = Vec::new();
        let mut counts = Counts::default();
        self.histories.retain(|&(protocol, tuple), history| {
            history.records.retain_mut(|record| {
                let life = record.life(now);
                if life.ended()
                    && !record.pending.active()
                    && record
                        .introduced
                        .is_some_and(|at| now >= at.saturating_add(highlight.duration()))
                {
                    let end = record.end.map_or_else(
                        || record.evidence.saturating_add(UDP_IDLE),
                        |window| window.end,
                    );
                    history.retired_through = Some(
                        history
                            .retired_through
                            .map_or(end, |previous| previous.max(end)),
                    );
                    return false;
                }
                let introduced = *record.introduced.get_or_insert(now);
                match life {
                    Life::Tcp(_) => counts.tcp += 1,
                    Life::EndedTcp => counts.ended_tcp += 1,
                    Life::Udp { .. } => counts.udp += 1,
                    Life::ExpiredUdp { .. } => counts.inactive_udp += 1,
                }
                let bytes = if elapsed.is_zero() {
                    Bytes::default()
                } else {
                    std::mem::take(&mut record.pending)
                };
                let rates = (complete && !record.uncertain && !record.gap)
                    .then(|| bytes.rates(elapsed))
                    .flatten();
                if rates.is_some_and(|rates| rates.received > 0.0 || rates.sent > 0.0) {
                    counts.transferring += 1;
                }
                observations.push(Observation {
                    id: record.id,
                    tuple,
                    life,
                    introduced,
                    bytes,
                    rates,
                });
                if !elapsed.is_zero() {
                    record.uncertain = false;
                }
                true
            });
            if history.records.is_empty() {
                let key = (protocol, tuple.local, tuple.remote);
                if let Some(processes) = self.endpoints.get_mut(&key) {
                    processes.remove(&tuple.process);
                    if processes.is_empty() {
                        self.endpoints.remove(&key);
                    }
                }
                false
            } else {
                true
            }
        });
        observations.sort_unstable_by_key(|observation| observation.id);
        self.published = now;
        let bytes = if elapsed.is_zero() {
            Bytes::default()
        } else {
            std::mem::take(&mut self.pending)
        };
        Snapshot {
            counts,
            observations: observations.into(),
            interval,
            coverage,
            bytes,
            unattributed: if elapsed.is_zero() {
                Bytes::default()
            } else {
                std::mem::take(&mut self.unattributed)
            },
            total_bytes: self.total,
            rates: complete.then(|| bytes.rates(elapsed)).flatten(),
            ambiguous_events: if elapsed.is_zero() {
                0
            } else {
                std::mem::take(&mut self.ambiguous)
            },
        }
    }
}

#[cfg(test)]
mod tests;

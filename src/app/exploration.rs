use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    net::{IpAddr, SocketAddr, SocketAddrV6},
    sync::Arc,
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{HFONT, InvalidateRect},
        UI::{Controls::*, HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::{HSTRING, PWSTR, Result, w},
};

use crate::{
    connections::{Endpoint, Life, ObservationId, ProcessId, Snapshot, TcpState},
    map::model::{Model, Place},
};

const COLUMNS: [(&str, i32); 9] = [
    ("Application", 150),
    ("PID", 65),
    ("Protocol", 65),
    ("Local endpoint", 190),
    ("Remote endpoint", 190),
    ("State / last seen", 190),
    ("Location / precision", 240),
    ("Receive", 100),
    ("Send", 100),
];

#[derive(Debug)]
struct Row {
    id: ObservationId,
    cells: [Box<[u16]>; COLUMNS.len()],
    place: Option<Place>,
}

/// Prepared on the existing scene worker from the same snapshot and model.
/// Cell callbacks only copy ready text; they never locate or inspect a process.
#[derive(Debug)]
pub(crate) struct Rows {
    rows: Vec<Row>,
    places: HashMap<Place, Arc<[usize]>>,
    located: bool,
}

impl Rows {
    /// Retain only previously located identities while the new lookup is pending.
    /// This runs on the collector worker; it does not assign locations to new IDs.
    pub(crate) fn prepare_pending(
        snapshot: &Snapshot,
        names: &HashMap<ProcessId, Box<str>>,
        previous: Option<&Rows>,
    ) -> Self {
        let mut pending = Self::prepare(snapshot, None, names);
        let Some(previous) = previous else {
            return pending;
        };
        let positions: HashMap<_, _> = pending
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.id, index))
            .collect();
        for (place, indices) in &previous.places {
            let mut surviving: Vec<_> = indices
                .iter()
                .filter_map(|&index| positions.get(&previous.rows[index].id).copied())
                .collect();
            surviving.sort_unstable();
            if !surviving.is_empty() {
                pending.places.insert(place.clone(), Arc::from(surviving));
            }
        }
        for row in &mut pending.rows {
            row.place = previous.place(row.id);
        }
        pending
    }

    pub(crate) fn prepare(
        snapshot: &Snapshot,
        model: Option<&Model>,
        names: &HashMap<ProcessId, Box<str>>,
    ) -> Self {
        let mut locations = HashMap::new();
        if let Some(model) = model {
            for group in &model.groups {
                let precision = if group.estimated {
                    "Country estimate"
                } else {
                    "Approximate coordinate"
                };
                let text = format!("{} · {precision}", group.name());
                for &id in &group.observations {
                    let entry = locations
                        .entry(id)
                        .or_insert_with(|| (group.place.clone(), text.clone()));
                    if group.place.cities {
                        *entry = (group.place.clone(), text.clone());
                    }
                }
            }
        }
        let mut rows: Vec<_> = snapshot
            .observations
            .iter()
            .map(|observation| {
                let (place, location) =
                    locations.get(&observation.id).map_or_else(
                        || {
                            (
                            None,
                            super::work::scene::unplottable(observation.tuple.remote.address())
                                .map_or_else(
                                    || if model.is_some() {
                                        "Geographic location unavailable in the current dataset"
                                    } else {
                                        "Locations updating…"
                                    },
                                    super::work::scene::Unplottable::description,
                                ).to_string(),
                        )
                        },
                        |(place, text)| (Some(place.clone()), text.clone()),
                    );
                let (protocol, state) = match observation.life {
                    Life::Tcp(state) => ("TCP", tcp_state(state).to_string()),
                    Life::EndedTcp => ("TCP", "Ended".to_string()),
                    Life::Udp { last_seen } | Life::ExpiredUdp { last_seen } => (
                        "UDP",
                        format!(
                            "{} · last seen {:.1} s ago",
                            if matches!(observation.life, Life::ExpiredUdp { .. }) {
                                "Flow expired"
                            } else {
                                "Observed flow"
                            },
                            snapshot
                                .interval
                                .end
                                .saturating_sub(last_seen)
                                .as_secs_f64()
                        ),
                    ),
                };
                let received = observation.rates.map_or_else(
                    || "Unavailable".to_string(),
                    |rates| format!("{:.0} B/s", rates.received),
                );
                let sent = observation.rates.map_or_else(
                    || "Unavailable".to_string(),
                    |rates| format!("{:.0} B/s", rates.sent),
                );
                Row {
                    id: observation.id,
                    place,
                    cells: [
                        names
                            .get(&observation.tuple.process)
                            .map_or("Name unavailable", AsRef::as_ref)
                            .to_string(),
                        observation.tuple.process.pid.to_string(),
                        protocol.to_string(),
                        endpoint(observation.tuple.local),
                        endpoint(observation.tuple.remote),
                        state,
                        location,
                        received,
                        sent,
                    ]
                    .map(|text| text.encode_utf16().collect::<Vec<_>>().into_boxed_slice()),
                }
            })
            .collect();
        rows.sort_unstable_by_key(|row| row.id);
        let mut places = HashMap::new();
        if let Some(model) = model {
            let positions: HashMap<_, _> = rows
                .iter()
                .enumerate()
                .map(|(index, row)| (row.id, index))
                .collect();
            for group in &model.groups {
                let mut indices: Vec<_> = group
                    .observations
                    .iter()
                    .filter_map(|id| positions.get(id).copied())
                    .collect();
                indices.sort_unstable();
                indices.dedup();
                places.insert(group.place.clone(), Arc::from(indices));
            }
        }
        Self {
            rows,
            places,
            located: model.is_some(),
        }
    }

    pub(crate) fn place(&self, id: ObservationId) -> Option<Place> {
        self.rows
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .and_then(|index| self.rows[index].place.clone())
    }
}

fn endpoint(endpoint: Endpoint) -> String {
    match endpoint.address() {
        IpAddr::V4(ip) => SocketAddr::new(IpAddr::V4(ip), endpoint.port()).to_string(),
        IpAddr::V6(ip) => SocketAddrV6::new(ip, endpoint.port(), 0, endpoint.scope()).to_string(),
    }
}

fn tcp_state(state: TcpState) -> &'static str {
    match state {
        TcpState::Closed => "Closed",
        TcpState::SynSent => "SYN sent",
        TcpState::SynReceived => "SYN received",
        TcpState::Established => "Established",
        TcpState::FinWait1 => "FIN wait 1",
        TcpState::FinWait2 => "FIN wait 2",
        TcpState::CloseWait => "Close wait",
        TcpState::Closing => "Closing",
        TcpState::LastAck => "Last ACK",
        TcpState::TimeWait => "Time wait",
        TcpState::DeleteTcb => "Delete TCB",
    }
}

struct View {
    rows: Arc<Rows>,
    place: Option<Place>,
    indices: Option<Arc<[usize]>>,
}

impl View {
    fn new(rows: Arc<Rows>, place: Option<&Place>) -> Self {
        let indices = place.map(|place| rows.places.get(place).cloned().unwrap_or_default());
        Self {
            rows,
            place: place.cloned(),
            indices,
        }
    }

    fn len(&self) -> usize {
        self.indices
            .as_ref()
            .map_or(self.rows.rows.len(), |indices| indices.len())
    }

    fn row(&self, index: usize) -> Option<&Row> {
        let index = match &self.indices {
            Some(indices) => *indices.get(index)?,
            None => index,
        };
        self.rows.rows.get(index)
    }

    fn position(&self, id: ObservationId) -> Option<usize> {
        match &self.indices {
            Some(indices) => indices
                .binary_search_by_key(&id, |&index| self.rows.rows[index].id)
                .ok(),
            None => self.rows.rows.binary_search_by_key(&id, |row| row.id).ok(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Selected(Option<ObservationId>),
    Clear,
}

pub(crate) struct Exploration {
    window: HWND,
    view: RefCell<Option<View>>,
    selected: Cell<Option<ObservationId>>,
    updating: Cell<bool>,
    dpi: Cell<u32>,
}

impl Exploration {
    pub(crate) fn create(parent: HWND, id: u16) -> Result<Self> {
        // SAFETY: A native UI-thread child; the parent owns HWND destruction.
        let window = unsafe {
            CreateWindowExW(
                WS_EX_CLIENTEDGE,
                WC_LISTVIEWW,
                w!("Connections"),
                WS_CHILD
                    | WS_TABSTOP
                    | WINDOW_STYLE(LVS_REPORT | LVS_OWNERDATA | LVS_SINGLESEL | LVS_SHOWSELALWAYS),
                0,
                0,
                0,
                0,
                Some(parent),
                Some(HMENU(id as usize as *mut _)),
                None,
                None,
            )?
        };
        // SAFETY: DPI is read from the live child on its creating UI thread.
        let dpi = unsafe { GetDpiForWindow(window) }.max(96);
        // SAFETY: The control copies each column label during this call.
        unsafe {
            SendMessageW(window, LVM_SETUNICODEFORMAT, Some(WPARAM(1)), None);
            SendMessageW(
                window,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                Some(WPARAM(
                    (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as usize,
                )),
                Some(LPARAM(
                    (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
                )),
            );
            for (index, (label, width)) in COLUMNS.iter().enumerate() {
                let text = HSTRING::from(*label);
                let column = LVCOLUMNW {
                    mask: LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM,
                    cx: *width * dpi as i32 / 96,
                    pszText: PWSTR(text.as_ptr() as *mut _),
                    iSubItem: index as i32,
                    ..Default::default()
                };
                SendMessageW(
                    window,
                    LVM_INSERTCOLUMNW,
                    Some(WPARAM(index)),
                    Some(LPARAM(&column as *const _ as isize)),
                );
            }
        }
        Ok(Self {
            window,
            view: RefCell::new(None),
            selected: Cell::new(None),
            updating: Cell::new(false),
            dpi: Cell::new(dpi),
        })
    }

    pub(crate) fn window(&self) -> HWND {
        self.window
    }

    pub(crate) fn selected(&self) -> Option<ObservationId> {
        self.selected.get()
    }

    pub(crate) fn font(&self, font: HFONT) {
        // SAFETY: Font is retained by Host for the lifetime of this control.
        unsafe {
            let dpi = GetDpiForWindow(self.window).max(96);
            let previous = self.dpi.replace(dpi);
            if previous != dpi {
                for index in 0..COLUMNS.len() {
                    let width =
                        SendMessageW(self.window, LVM_GETCOLUMNWIDTH, Some(WPARAM(index)), None).0;
                    SendMessageW(
                        self.window,
                        LVM_SETCOLUMNWIDTH,
                        Some(WPARAM(index)),
                        Some(LPARAM(width * dpi as isize / previous as isize)),
                    );
                }
            }
            SendMessageW(
                self.window,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }

    pub(crate) fn clear_selection(&self) {
        let selected = self.selected.take();
        let index = selected.and_then(|id| self.view.borrow().as_ref()?.position(id));
        if let Some(index) = index {
            self.updating.set(true);
            // SAFETY: Explicit clear changes this UI-thread control's selection.
            unsafe {
                self.item_state(index, false);
            }
            self.updating.set(false);
        }
    }

    pub(crate) fn filter(
        &self,
        place: Option<&Place>,
    ) -> (Option<ObservationId>, Option<Arc<Rows>>) {
        let rows = self
            .view
            .borrow()
            .as_ref()
            .map(|view| Arc::clone(&view.rows));
        if let Some(rows) = rows {
            self.set_rows(rows, place)
        } else {
            (self.selected(), None)
        }
    }

    pub(crate) fn set_rows(
        &self,
        rows: Arc<Rows>,
        place: Option<&Place>,
    ) -> (Option<ObservationId>, Option<Arc<Rows>>) {
        if self
            .view
            .borrow()
            .as_ref()
            .is_some_and(|view| Arc::ptr_eq(&view.rows, &rows) && view.place.as_ref() == place)
        {
            return (self.selected(), None);
        }
        let next = View::new(rows, place);
        let selected = self.selected.get();
        let next_index = selected.and_then(|id| next.position(id));
        let old_index = selected.and_then(|id| self.view.borrow().as_ref()?.position(id));
        let count = next.len();
        self.updating.set(true);
        // Swap before native calls: notifications can synchronously request cells.
        let replaced = self.view.replace(Some(next));
        self.selected.set(next_index.and(selected));
        // SAFETY: Only this UI thread mutates the control. All borrows end before
        // calls which can synchronously deliver notifications to the parent.
        unsafe {
            if let Some(index) = old_index {
                self.item_state(index, false);
            }
            SendMessageW(
                self.window,
                LVM_SETITEMCOUNT,
                Some(WPARAM(count)),
                Some(LPARAM(LVSICF_NOSCROLL as isize)),
            );
            if let Some(index) = next_index {
                self.item_state(index, true);
            }
            let _ = InvalidateRect(Some(self.window), None, false);
        }
        self.updating.set(false);
        (self.selected(), replaced.map(|view| view.rows))
    }

    /// Drain plain row ownership before the native host and control disappear.
    pub(crate) fn take_rows(&self) -> Option<Arc<Rows>> {
        self.selected.set(None);
        self.view.take().map(|view| view.rows)
    }

    unsafe fn item_state(&self, index: usize, selected: bool) {
        let state = LVITEMW {
            stateMask: LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0),
            state: if selected {
                LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0)
            } else {
                LIST_VIEW_ITEM_STATE_FLAGS(0)
            },
            ..Default::default()
        };
        // SAFETY: Synchronous state update on this live UI-thread control.
        unsafe {
            SendMessageW(
                self.window,
                LVM_SETITEMSTATE,
                Some(WPARAM(index)),
                Some(LPARAM(&state as *const _ as isize)),
            );
        }
    }

    /// Caller forwards WM_NOTIFY only while its lParam notification is live.
    pub(crate) unsafe fn notify(&self, header: *mut NMHDR) -> Option<(LRESULT, Option<Event>)> {
        if header.is_null() {
            return None;
        }
        // SAFETY: Caller guarantees a live WM_NOTIFY header and matching payload.
        let header_ref = unsafe { &*header };
        if header_ref.hwndFrom != self.window {
            return None;
        }
        match header_ref.code {
            LVN_GETDISPINFOW => {
                // SAFETY: This notification code defines NMLVDISPINFOW.
                let info = unsafe { &mut *header.cast::<NMLVDISPINFOW>() };
                if info.item.mask.contains(LVIF_TEXT)
                    && info.item.cchTextMax > 0
                    && !info.item.pszText.is_null()
                {
                    let view = self.view.borrow();
                    let text = view
                        .as_ref()
                        .and_then(|view| view.row(info.item.iItem as usize))
                        .and_then(|row| row.cells.get(info.item.iSubItem as usize));
                    let text = text.map_or(&[][..], AsRef::as_ref);
                    let capacity = (info.item.cchTextMax as usize).min(text.len() + 1);
                    // SAFETY: The native control supplies cchTextMax writable
                    // units; borrow only the bounded prefix this cell can fill.
                    let output =
                        unsafe { std::slice::from_raw_parts_mut(info.item.pszText.0, capacity) };
                    copy_text(text, output);
                }
                Some((LRESULT(0), None))
            }
            LVN_GETEMPTYMARKUP => {
                // SAFETY: This notification code defines NMLVEMPTYMARKUP.
                let info = unsafe { &mut *header.cast::<NMLVEMPTYMARKUP>() };
                let view = self.view.borrow();
                let text = match view.as_ref() {
                    None => "Preparing connections…",
                    Some(view) if view.place.is_some() && !view.rows.located => {
                        "Locations updating…"
                    }
                    Some(view) if view.place.is_some() => {
                        "No retained observations at this place. Use All connections to clear the filter."
                    }
                    _ => "No observed connections.",
                };
                for (output, value) in info
                    .szMarkup
                    .iter_mut()
                    .zip(text.encode_utf16().chain(std::iter::once(0)))
                {
                    *output = value;
                }
                Some((LRESULT(1), None))
            }
            LVN_ITEMCHANGED if !self.updating.get() => {
                // SAFETY: Query bounded native interaction state, never the facts.
                let index = unsafe {
                    SendMessageW(
                        self.window,
                        LVM_GETNEXTITEM,
                        Some(WPARAM(usize::MAX)),
                        Some(LPARAM(LVNI_SELECTED as isize)),
                    )
                }
                .0;
                let selected = (index >= 0)
                    .then(|| {
                        self.view
                            .borrow()
                            .as_ref()
                            .and_then(|view| view.row(index as usize).map(|row| row.id))
                    })
                    .flatten();
                let changed = self.selected.replace(selected) != selected;
                Some((LRESULT(0), changed.then_some(Event::Selected(selected))))
            }
            LVN_KEYDOWN => {
                // SAFETY: Packed Win32 notification; read the key by value.
                let key = unsafe {
                    std::ptr::addr_of!((*header.cast::<NMLVKEYDOWN>()).wVKey).read_unaligned()
                };
                Some((LRESULT(0), (key == 0x1b).then_some(Event::Clear)))
            }
            _ => None,
        }
    }
}

fn copy_text(text: &[u16], output: &mut [u16]) {
    if output.is_empty() {
        return;
    }
    let mut length = text.len().min(output.len() - 1);
    if length > 0 && (0xd800..=0xdbff).contains(&text[length - 1]) {
        length -= 1;
    }
    output[..length].copy_from_slice(&text[..length]);
    output[length] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        connections::{Bytes, Counts, Coverage, Observation, Tuple, Window},
        geography::{Country, Point, World},
        geolocation::Location,
        map::model::Located,
    };
    use std::time::Duration;

    fn observation(id: u64, life: Life) -> Observation {
        Observation {
            id: ObservationId(id),
            tuple: Tuple {
                process: ProcessId {
                    pid: 17,
                    generation: 2,
                },
                local: Endpoint::new("127.0.0.1:1234".parse().unwrap()),
                remote: Endpoint::new("8.8.8.8:443".parse().unwrap()),
            },
            life,
            introduced: Duration::ZERO,
            bytes: Bytes::default(),
            rates: None,
        }
    }

    fn snapshot(observations: Vec<Observation>) -> Snapshot {
        Snapshot {
            observations: observations.into(),
            counts: Counts::default(),
            interval: Window {
                begin: Duration::ZERO,
                end: Duration::from_secs(3),
            },
            coverage: Coverage::Complete,
            bytes: Bytes::default(),
            unattributed: Bytes::default(),
            total_bytes: Bytes::default(),
            rates: None,
            ambiguous_events: 0,
        }
    }

    #[test]
    fn ended_selection_never_moves_to_a_same_tuple_reconnect() {
        let rows = Arc::new(Rows::prepare(
            &snapshot(vec![
                observation(4, Life::Tcp(TcpState::Established)),
                observation(3, Life::EndedTcp),
            ]),
            None,
            &HashMap::new(),
        ));
        let view = View::new(rows, None);
        assert_eq!(
            view.row(view.position(ObservationId(3)).unwrap())
                .unwrap()
                .id,
            ObservationId(3)
        );
        let next = View::new(
            Arc::new(Rows::prepare(
                &snapshot(vec![observation(4, Life::Tcp(TcpState::Established))]),
                None,
                &HashMap::new(),
            )),
            None,
        );
        assert_eq!(next.position(ObservationId(3)), None);
        assert_eq!(next.row(0).unwrap().id, ObservationId(4));
    }

    #[test]
    fn place_filters_use_authoritative_membership_and_all_keeps_unplottable_rows() {
        let facts = snapshot(vec![
            observation(1, Life::EndedTcp),
            observation(2, Life::Tcp(TcpState::Established)),
        ]);
        let point = Point {
            longitude: 10.0,
            latitude: 20.0,
        };
        let world = World {
            sections: Box::new([]),
            countries: vec![Country {
                code: "US".into(),
                anchor: point,
                rings: Box::new([]),
                areas: Box::new([]),
            }]
            .into_boxed_slice(),
        };
        let location = Location {
            code: "US".into(),
            country: "United States".into(),
            city: None,
            point: None,
        };
        let model = Model::arrange(
            &world,
            facts
                .observations
                .iter()
                .enumerate()
                .map(|(index, observation)| Located {
                    observation,
                    location: (index == 0).then_some(&location),
                }),
            false,
            0.0,
            1.0,
        );
        let rows = Arc::new(Rows::prepare(&facts, Some(&model), &HashMap::new()));
        assert_eq!(View::new(Arc::clone(&rows), None).len(), 2);
        let filtered = View::new(Arc::clone(&rows), Some(&model.groups[0].place));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.row(0).unwrap().id, ObservationId(1));
        assert!(rows.place(ObservationId(2)).is_none());
        let pending = View::new(
            Arc::new(Rows::prepare(&facts, None, &HashMap::new())),
            Some(&model.groups[0].place),
        );
        assert_eq!(pending.len(), 0);
        assert!(!pending.rows.located);

        // A location refresh keeps a surviving selected identity's membership,
        // but never assigns the same-tuple successor to the old place.
        let updated = snapshot(vec![
            observation(1, Life::EndedTcp),
            observation(3, Life::Tcp(TcpState::Established)),
        ]);
        let pending = Arc::new(Rows::prepare_pending(
            &updated,
            &HashMap::new(),
            Some(&rows),
        ));
        let filtered = View::new(Arc::clone(&pending), Some(&model.groups[0].place));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.position(ObservationId(1)), Some(0));
        assert_eq!(filtered.position(ObservationId(3)), None);
        assert_eq!(
            pending.place(ObservationId(1)),
            Some(model.groups[0].place.clone())
        );
        assert_eq!(pending.place(ObservationId(3)), None);
        assert!(!pending.located);
        assert_eq!(View::new(pending, None).len(), 2);

        let retired = snapshot(vec![observation(3, Life::Tcp(TcpState::Established))]);
        let pending = Arc::new(Rows::prepare_pending(
            &retired,
            &HashMap::new(),
            Some(&rows),
        ));
        let filtered = View::new(Arc::clone(&pending), Some(&model.groups[0].place));
        assert_eq!(filtered.position(ObservationId(1)), None);
        assert_eq!(filtered.len(), 0);
        assert_eq!(View::new(pending, None).len(), 1);
    }
}

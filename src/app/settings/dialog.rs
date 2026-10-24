// Native shell/search/page mechanisms adapted from Goz. See Goz-MIT.txt.
use super::*;
use crate::{
    app::Application,
    host::{controls as control, dialog as native, ids::*},
};
use std::cell::{Cell, RefCell};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        UI::{
            Controls::*,
            Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_CONTROL, VK_F, VK_RETURN},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    core::{BOOL, HSTRING, Result},
};

pub const REFRESH: u32 = WM_APP + 3;
const SELECT_PAGE: u32 = WM_APP + 4;
mod layout;

#[derive(Clone, Copy)]
enum Intent {
    Apply,
    Ok,
    Observe,
}

enum Editing {
    Waiting,
    Ready,
    Saving { id: u64, intent: Intent },
}

struct Model {
    draft: Draft,
    baseline: Draft,
    editing: Editing,
    invalid: Option<Invalid>,
}

struct Dialog<'a> {
    app: &'a Application,
    id: u64,
    window: Cell<HWND>,
    pages: [Cell<HWND>; 5],
    shown: Cell<Option<Page>>,
    last: Cell<Page>,
    listed: RefCell<Vec<Page>>,
    filling: Cell<bool>,
    model: RefCell<Model>,
    fonts: Cell<Option<native::Fonts>>,
    navigation_width: Cell<i32>,
    monitor_choices: RefCell<Vec<(String, String)>>,
}

pub fn show(app: &Application, owner: HWND, page: Page) -> Result<()> {
    if let Some((id, existing)) = app.settings_dialog.get() {
        // SAFETY: Application's live-dialog registration is UI-thread owned.
        unsafe {
            let _ = PostMessageW(
                Some(existing),
                SELECT_PAGE,
                WPARAM(id as usize),
                LPARAM(index(page) as isize),
            );
            let _ = SetForegroundWindow(existing);
        }
        return Ok(());
    }
    let id = app.dialog_generation.get() + 1;
    app.dialog_generation.set(id);
    let preferences = app.preferences();
    let draft = Draft::from(&preferences.committed);
    let dialog = Box::new(Dialog {
        app,
        id,
        window: Cell::new(HWND::default()),
        pages: std::array::from_fn(|_| Cell::new(HWND::default())),
        shown: Cell::new(None),
        last: Cell::new(page),
        listed: RefCell::new(Page::ALL.to_vec()),
        filling: Cell::new(false),
        fonts: Cell::new(None),
        navigation_width: Cell::new(0),
        monitor_choices: RefCell::new(Vec::new()),
        model: RefCell::new(Model {
            baseline: draft.clone(),
            draft,
            editing: Editing::Waiting,
            invalid: None,
        }),
    });
    native::modal(
        owner,
        IDD_SETTINGS,
        Some(shell_proc),
        LPARAM((&*dialog as *const Dialog) as isize),
    )
}

fn index(page: Page) -> usize {
    match page {
        Page::Map => 0,
        Page::Connections => 1,
        Page::Location => 2,
        Page::Desktop => 3,
        Page::ScreenSaver => 4,
    }
}

fn template(page: Page) -> u16 {
    match page {
        Page::Map => IDD_SETTINGS_MAP,
        Page::Connections => IDD_SETTINGS_CONNECTIONS,
        Page::Location => IDD_SETTINGS_LOCATION,
        Page::Desktop => IDD_SETTINGS_DESKTOP,
        Page::ScreenSaver => IDD_SETTINGS_SAVER,
    }
}

fn captions(page: Page) -> &'static [(u16, &'static str)] {
    match page {
        Page::Map => &[
            (IDC_SECTION, "display"),
            (IDC_DETAIL_LABEL, "detail"),
            (IDC_SPEEDS_LABEL, "speeds"),
            (IDC_NOTE, "appearance"),
            (IDC_LOCATION_SETTINGS, "location"),
        ],
        Page::Connections => &[
            (IDC_SECTION, "new_connections"),
            (IDC_BRIGHT_LABEL, "bright"),
            (IDC_FADE_LABEL, "fade"),
            (IDC_TRAFFIC, "traffic"),
            (IDC_NOTE, "activity"),
        ],
        Page::Location => &[
            (IDC_SECTION, "map_origin"),
            (IDC_AUTOMATIC, "automatic"),
            (IDC_MANUAL, "manual"),
            (IDC_LATITUDE_LABEL, "latitude"),
            (IDC_LONGITUDE_LABEL, "longitude"),
            (IDC_NOTE, "disclosure"),
            (IDC_LOCATION_DATA, "location_data"),
            (IDC_GET_CITY, "get_city"),
            (IDC_CANCEL_CITY_DATA, "cancel_city"),
            (IDC_IMMEDIATE, "immediate"),
        ],
        Page::Desktop => &[
            (IDC_SECTION, "background"),
            (IDC_DESKTOP_ENABLED, "enable"),
            (IDC_MONITOR_LABEL, "monitor"),
            (IDC_NOTE, "behavior"),
        ],
        Page::ScreenSaver => &[
            (IDC_SECTION, "installation"),
            (IDC_INSTALL_SAVER, "install"),
            (IDC_SAVER_WINDOWS, "windows"),
            (IDC_PREVIEW_SAVER, "preview"),
            (IDC_IMMEDIATE, "immediate"),
        ],
    }
}

struct Fill<'a>(&'a Cell<bool>, bool);
impl<'a> Fill<'a> {
    fn new(cell: &'a Cell<bool>) -> Self {
        Self(cell, cell.replace(true))
    }
}
impl Drop for Fill<'_> {
    fn drop(&mut self) {
        self.0.set(self.1);
    }
}

impl Dialog<'_> {
    fn item(&self, id: u16) -> HWND {
        control::item(self.window.get(), id)
    }
    fn page(&self, page: Page) -> HWND {
        self.pages[index(page)].get()
    }
    fn can_edit(&self) -> bool {
        matches!(self.model.borrow().editing, Editing::Ready)
    }

    fn initialize(&self) {
        let window = self.window.get();
        self.app.settings_dialog.set(Some((self.id, window)));
        // SAFETY: Resource-created controls and bounded native structures live
        // through these synchronous initialization calls on the UI thread.
        unsafe {
            SendMessageW(
                self.item(IDC_SEARCH),
                EM_SETLIMITTEXT,
                Some(WPARAM(128)),
                None,
            );
            let cue = HSTRING::from("Search settings");
            SendMessageW(
                self.item(IDC_SEARCH),
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(cue.as_ptr() as isize)),
            );
            SendMessageW(
                self.item(IDC_PAGES),
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                Some(WPARAM(LVS_EX_DOUBLEBUFFER as usize)),
                Some(LPARAM(LVS_EX_DOUBLEBUFFER as isize)),
            );
            let column = LVCOLUMNW {
                mask: LVCF_WIDTH,
                cx: native::units(window, 0, 0, 80, 0).right,
                ..Default::default()
            };
            SendMessageW(
                self.item(IDC_PAGES),
                LVM_INSERTCOLUMNW,
                Some(WPARAM(0)),
                Some(LPARAM(&column as *const _ as isize)),
            );
        }
        self.update_fonts();
        native::fit_work_area(window, None);
        self.filter();
        self.refresh();
        self.layout();
        // SAFETY: Initial category selection exists before its focus effect.
        unsafe {
            let _ = SetFocus(Some(self.item(IDC_PAGES)));
        }
    }

    fn update_fonts(&self) {
        let Ok(fonts) = native::Fonts::new(self.window.get()) else {
            return;
        };
        let old = self.fonts.replace(Some(fonts));
        native::font(self.window.get(), fonts.normal);
        self.navigation_width.set(
            native::text_width(self.window.get(), fonts.normal, &Page::ALL.map(Page::title))
                + native::units(self.window.get(), 0, 0, 16, 0).right,
        );
        // SAFETY: Callback receives a copied font handle that remains feature-
        // owned until all descendants have received the replacement font.
        unsafe {
            let _ = EnumChildWindows(
                Some(self.window.get()),
                Some(set_font),
                LPARAM(fonts.normal.0 as isize),
            );
        }
        self.highlight();
        if let Some(old) = old {
            old.release();
        }
    }

    fn make_page(&self, page: Page) {
        if !self.page(page).is_invalid() {
            return;
        }
        let Ok(window) = native::page(
            self.window.get(),
            template(page),
            Some(page_proc),
            LPARAM(self as *const Self as isize),
        ) else {
            return;
        };
        self.pages[index(page)].set(window);
        // SAFETY: Place the DS_CONTROL page after navigation in native tab
        // order, before the common reset/footer controls.
        unsafe {
            let _ = SetWindowPos(
                window,
                Some(self.item(IDC_PAGES)),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        control::set_text(control::item(window, IDC_HEADING), page.title());
        for (id, key) in captions(page) {
            control::set_text(control::item(window, *id), page.caption(key));
        }
        // SAFETY: These controls are resource-defined and copy captions/data.
        unsafe {
            if page == Page::Map {
                for (id, keys) in [
                    (IDC_DETAIL, &["countries", "cities"][..]),
                    (IDC_SPEEDS, &["top_left", "bottom_left", "bottom_right"][..]),
                ] {
                    for key in keys {
                        let caption = HSTRING::from(page.caption(key));
                        SendMessageW(
                            control::item(window, id),
                            CB_ADDSTRING,
                            None,
                            Some(LPARAM(caption.as_ptr() as isize)),
                        );
                    }
                    SendMessageW(
                        control::item(window, id),
                        CB_SETMINVISIBLE,
                        Some(WPARAM(keys.len())),
                        None,
                    );
                }
            }
            for id in [IDC_BRIGHT, IDC_FADE, IDC_LATITUDE, IDC_LONGITUDE] {
                let edit = control::item(window, id);
                if !edit.is_invalid() {
                    SendMessageW(edit, EM_SETLIMITTEXT, Some(WPARAM(128)), None);
                }
            }
            if page == Page::Connections {
                for (spin, buddy) in [(IDC_BRIGHT_SPIN, IDC_BRIGHT), (IDC_FADE_SPIN, IDC_FADE)] {
                    SendMessageW(
                        control::item(window, spin),
                        UDM_SETBUDDY,
                        Some(WPARAM(control::item(window, buddy).0 as usize)),
                        None,
                    );
                }
            }
            let _ = EnumChildWindows(
                Some(window),
                Some(install_keys),
                LPARAM(self as *const Self as isize),
            );
        }
        if let Some(fonts) = self.fonts.get() {
            native::font(window, fonts.normal);
            // SAFETY: Font remains owned throughout the child page lifetime.
            unsafe {
                let _ = EnumChildWindows(
                    Some(window),
                    Some(set_font),
                    LPARAM(fonts.normal.0 as isize),
                );
            }
        }
        self.fill_page(page);
        self.highlight();
    }

    fn fill_page(&self, page: Page) {
        let window = self.page(page);
        if window.is_invalid() {
            return;
        }
        let draft = self.model.borrow().draft.clone();
        let _fill = Fill::new(&self.filling);
        // SAFETY: Explicit draft-to-control fill; the guard ignores its own
        // notifications and no model borrow spans any native call.
        unsafe {
            match page {
                Page::Map => {
                    SendMessageW(
                        control::item(window, IDC_DETAIL),
                        CB_SETCURSEL,
                        Some(WPARAM(usize::from(draft.detail == Detail::Cities))),
                        None,
                    );
                    let corner = match draft.speeds {
                        SpeedCorner::TopLeft => 0,
                        SpeedCorner::BottomLeft => 1,
                        SpeedCorner::BottomRight => 2,
                    };
                    SendMessageW(
                        control::item(window, IDC_SPEEDS),
                        CB_SETCURSEL,
                        Some(WPARAM(corner)),
                        None,
                    );
                }
                Page::Connections => {
                    control::set_text(control::item(window, IDC_BRIGHT), &draft.bright);
                    control::set_text(control::item(window, IDC_FADE), &draft.fade);
                }
                Page::Location => {
                    for (id, checked) in
                        [(IDC_AUTOMATIC, !draft.manual), (IDC_MANUAL, draft.manual)]
                    {
                        SendMessageW(
                            control::item(window, id),
                            BM_SETCHECK,
                            Some(WPARAM(usize::from(checked))),
                            None,
                        );
                    }
                    control::set_text(control::item(window, IDC_LATITUDE), &draft.latitude);
                    control::set_text(control::item(window, IDC_LONGITUDE), &draft.longitude);
                }
                Page::Desktop => {
                    SendMessageW(
                        control::item(window, IDC_DESKTOP_ENABLED),
                        BM_SETCHECK,
                        Some(WPARAM(usize::from(draft.desktop))),
                        None,
                    );
                    self.refresh_monitors();
                }
                Page::ScreenSaver => {}
            }
        }
    }

    fn refresh_monitors(&self) {
        let page = self.page(Page::Desktop);
        if page.is_invalid() {
            return;
        }
        let monitor = self.model.borrow().draft.monitor.clone();
        let mut choices = vec![(String::new(), Page::Desktop.caption("primary").to_owned())];
        for (name, primary) in self.app.desktop_monitors() {
            if name.is_empty() || choices.iter().any(|(id, _)| *id == name) {
                continue;
            }
            let caption = if primary {
                format!("{name} (primary)")
            } else {
                name.clone()
            };
            choices.push((name, caption));
        }
        if !choices.iter().any(|(name, _)| *name == monitor) {
            choices.push((monitor.clone(), format!("{monitor} (disconnected)")));
        }
        let selection = choices
            .iter()
            .position(|(name, _)| *name == monitor)
            .unwrap();
        let combo = control::item(page, IDC_MONITOR);
        let changed = *self.monitor_choices.borrow() != choices;
        let _fill = Fill::new(&self.filling);
        // SAFETY: This bounded current-display list is application-owned ready
        // data. The combo copies each string; no native call spans a model borrow.
        unsafe {
            if changed {
                self.monitor_choices.replace(choices.clone());
                SendMessageW(combo, CB_RESETCONTENT, None, None);
                for (_, caption) in choices {
                    let caption = HSTRING::from(caption);
                    SendMessageW(
                        combo,
                        CB_ADDSTRING,
                        None,
                        Some(LPARAM(caption.as_ptr() as isize)),
                    );
                }
                SendMessageW(combo, CB_SETMINVISIBLE, Some(WPARAM(8)), None);
            }
            if changed || SendMessageW(combo, CB_GETCURSEL, None, None).0 != selection as isize {
                SendMessageW(combo, CB_SETCURSEL, Some(WPARAM(selection)), None);
            }
        }
    }

    fn filter(&self) {
        let search = control::text(self.item(IDC_SEARCH));
        let pages = matching_pages(&search);
        let shown = self
            .shown
            .get()
            .filter(|page| pages.contains(page))
            .or_else(|| {
                if search.is_empty() {
                    Some(self.last.get()).filter(|page| pages.contains(page))
                } else {
                    None
                }
            })
            .or(pages.first().copied());
        self.listed.replace(pages.clone());
        self.shown.set(shown);
        if let Some(page) = shown {
            self.last.set(page);
            self.make_page(page);
        }
        {
            let _fill = Fill::new(&self.filling);
            control::list_pages(
                self.item(IDC_PAGES),
                &pages.iter().map(|page| page.title()).collect::<Vec<_>>(),
                shown.and_then(|page| pages.iter().position(|p| *p == page)),
            );
        }
        for page in Page::ALL {
            let window = self.page(page);
            if !window.is_invalid() {
                control::visible(window, Some(page) == shown);
            }
        }
        control::visible(self.item(IDC_NO_MATCH), shown.is_none());
        control::visible(
            self.item(IDC_RESET_PAGE),
            shown.is_some_and(|page| page != Page::ScreenSaver),
        );
        control::enable(self.item(IDC_CLEAR), !search.is_empty());
        self.highlight();
        self.layout();
        self.refresh();
    }

    fn highlight(&self) {
        let Some(fonts) = self.fonts.get() else {
            return;
        };
        let search = control::text(self.item(IDC_SEARCH));
        for page in Page::ALL {
            let window = self.page(page);
            if window.is_invalid() {
                continue;
            }
            native::font(control::item(window, IDC_HEADING), fonts.bold);
            for (id, key) in captions(page) {
                let heading = [IDC_SECTION, IDC_TRAFFIC, IDC_LOCATION_DATA].contains(id);
                native::font(
                    control::item(window, *id),
                    if heading || caption_matches(page.caption(key), &search) {
                        fonts.bold
                    } else {
                        fonts.normal
                    },
                );
            }
        }
    }

    fn changed(&self, source: HWND, id: u16) {
        if self.filling.get() || !self.can_edit() {
            return;
        }
        let page = self.shown.get().unwrap_or(Page::Map);
        let window = self.page(page);
        // Read the notifying control before borrowing the draft. Hidden pages
        // are never inspected as a second store of raw editor values.
        let text = control::text(source);
        // SAFETY: Selection read is bounded and synchronous on its UI thread.
        let selection = unsafe { SendMessageW(source, CB_GETCURSEL, None, None).0 };
        // SAFETY: The notifying resource control is live on this dialog's UI
        // thread; this synchronous check-state read retains no pointer or borrow.
        let checked = unsafe { SendMessageW(source, BM_GETCHECK, None, None).0 != 0 };
        let monitor = (id == IDC_MONITOR)
            .then(|| {
                self.monitor_choices
                    .borrow()
                    .get(selection as usize)
                    .cloned()
            })
            .flatten();
        {
            let mut model = self.model.borrow_mut();
            let draft = &mut model.draft;
            match id {
                IDC_DETAIL => {
                    draft.detail = if selection == 1 {
                        Detail::Cities
                    } else {
                        Detail::Countries
                    }
                }
                IDC_SPEEDS => {
                    draft.speeds = match selection {
                        0 => SpeedCorner::TopLeft,
                        1 => SpeedCorner::BottomLeft,
                        _ => SpeedCorner::BottomRight,
                    }
                }
                IDC_BRIGHT => draft.bright = text,
                IDC_FADE => draft.fade = text,
                IDC_LATITUDE => draft.latitude = text,
                IDC_LONGITUDE => draft.longitude = text,
                IDC_AUTOMATIC | IDC_MANUAL => draft.manual = id == IDC_MANUAL,
                IDC_DESKTOP_ENABLED => draft.desktop = checked,
                IDC_MONITOR => {
                    let Some((name, _)) = monitor else { return };
                    draft.monitor = name;
                }
                _ => return,
            }
            model.invalid = None;
        }
        if [IDC_AUTOMATIC, IDC_MANUAL].contains(&id) {
            // The native radio control owns the checked interaction state;
            // enabled coordinate editors derive from the accepted draft mode.
            let manual = self.model.borrow().draft.manual;
            control::enable(control::item(window, IDC_LATITUDE), manual);
            control::enable(control::item(window, IDC_LONGITUDE), manual);
        }
        self.refresh();
    }

    fn refresh(&self) {
        let settings = self.app.preferences();
        let (refill, close) = {
            let mut model = self.model.borrow_mut();
            match (&model.editing, &settings.phase) {
                (Editing::Waiting, Phase::Ready) => {
                    model.draft = Draft::from(&settings.committed);
                    model.baseline = model.draft.clone();
                    model.editing = Editing::Ready;
                    (true, false)
                }
                (Editing::Waiting, Phase::Saving { id, .. }) => {
                    model.editing = Editing::Saving {
                        id: *id,
                        intent: Intent::Observe,
                    };
                    (false, false)
                }
                (Editing::Saving { id, intent }, Phase::Ready)
                    if settings.last_save().is_some_and(|(done, _)| done == *id) =>
                {
                    let intent = *intent;
                    let success = settings.error.is_none();
                    if matches!(intent, Intent::Observe) {
                        model.draft = Draft::from(&settings.committed);
                    }
                    if success || matches!(intent, Intent::Observe) {
                        model.baseline = model.draft.clone();
                    }
                    model.editing = Editing::Ready;
                    (
                        matches!(intent, Intent::Observe),
                        success && matches!(intent, Intent::Ok),
                    )
                }
                _ => (false, false),
            }
        };
        if close {
            // SAFETY: Only this dialog's own successful OK request closes it.
            unsafe {
                let _ = EndDialog(self.window.get(), 1);
            }
            return;
        }
        if refill {
            for page in Page::ALL {
                self.fill_page(page);
            }
        }
        let (editing, dirty, reset, manual, invalid) = {
            let model = self.model.borrow();
            (
                matches!(model.editing, Editing::Ready),
                model.draft != model.baseline,
                self.shown
                    .get()
                    .is_some_and(|page| model.draft.differs_from_defaults(page)),
                model.draft.manual,
                model.invalid.as_ref().map(|invalid| invalid.reason),
            )
        };
        for page in Page::ALL {
            let window = self.page(page);
            if window.is_invalid() {
                continue;
            }
            let ids: &[u16] = match page {
                Page::Map => &[IDC_DETAIL, IDC_SPEEDS],
                Page::Connections => &[IDC_BRIGHT, IDC_BRIGHT_SPIN, IDC_FADE, IDC_FADE_SPIN],
                Page::Location => &[IDC_AUTOMATIC, IDC_MANUAL, IDC_LATITUDE, IDC_LONGITUDE],
                Page::Desktop => &[IDC_DESKTOP_ENABLED, IDC_MONITOR],
                Page::ScreenSaver => &[],
            };
            for id in ids {
                control::enable(
                    control::item(window, *id),
                    editing && (manual || ![IDC_LATITUDE, IDC_LONGITUDE].contains(id)),
                );
            }
        }
        control::enable(self.item(IDC_APPLY), editing && dirty);
        control::enable(self.item(IDOK.0 as u16), editing);
        control::enable(self.item(IDC_RESET_PAGE), editing && reset);
        let reset_all = self.model.borrow().draft != Draft::from(&Preferences::default());
        control::enable(self.item(IDC_RESET_ALL), editing && reset_all);
        let map = self.page(Page::Map);
        if !map.is_invalid() {
            let note = if self.app.city_available() {
                Page::Map.caption("appearance").to_owned()
            } else {
                format!(
                    "{}\r\n{}",
                    Page::Map.caption("appearance"),
                    Page::Map.caption("cities_unavailable")
                )
            };
            control::set_text(control::item(map, IDC_NOTE), &note);
        }
        let location = self.page(Page::Location);
        if !location.is_invalid() {
            control::set_text(
                control::item(location, IDC_DATA_STATUS),
                &self.app.location_status(),
            );
            control::set_text(
                control::item(location, IDC_GET_CITY),
                Page::Location.caption(if self.app.city_available() {
                    "update_city"
                } else {
                    "get_city"
                }),
            );
            control::enable(
                control::item(location, IDC_GET_CITY),
                self.app.can_download_city(),
            );
            control::enable(
                control::item(location, IDC_CANCEL_CITY_DATA),
                self.app.can_cancel_city(),
            );
            let origin = self.app.origin_status();
            control::set_text(control::item(location, IDC_ORIGIN_STATUS), &origin);
        }
        self.refresh_monitors();
        let saver = self.page(Page::ScreenSaver);
        if !saver.is_invalid() {
            control::set_text(
                control::item(saver, IDC_SAVER_STATUS),
                &self.app.screen_saver_status(),
            );
        }
        let state = invalid
            .map(str::to_owned)
            .or(settings.error)
            .unwrap_or_else(|| match settings.phase {
                Phase::Loading => "Loading preferences…".into(),
                Phase::Saving { .. } => "Saving…".into(),
                Phase::Ready => self.app.city_status().unwrap_or_else(|| {
                    if dirty {
                        "Unsaved changes".into()
                    } else {
                        "Ready".into()
                    }
                }),
            });
        control::status(
            self.item(IDC_SETTINGS_STATUS),
            0,
            &self
                .shown
                .get()
                .map(|page| format!("Settings - {}", page.title()))
                .unwrap_or_else(|| "Settings - no matches".into()),
        );
        control::status(self.item(IDC_SETTINGS_STATUS), 1, &state);
        self.layout_pages();
    }

    fn accept(&self, intent: Intent) {
        if !self.can_edit() {
            return;
        }
        let (draft, dirty) = {
            let model = self.model.borrow();
            (model.draft.clone(), model.draft != model.baseline)
        };
        if !dirty {
            if matches!(intent, Intent::Ok) {
                // SAFETY: No pending edits or save need this live dialog.
                unsafe {
                    let _ = EndDialog(self.window.get(), 1);
                }
            }
            return;
        }
        let committed = self.app.preferences().committed;
        match draft.validate(self.app.city_available(), &committed) {
            Ok(preferences) => {
                if let Some(id) = self.app.save_preferences(preferences) {
                    self.model.borrow_mut().editing = Editing::Saving { id, intent };
                }
                self.refresh();
            }
            Err(invalid) => {
                let field = invalid.field;
                self.model.borrow_mut().invalid = Some(invalid);
                self.select_page(field.page());
                let id = match field {
                    Field::Detail => IDC_DETAIL,
                    Field::Bright => IDC_BRIGHT,
                    Field::Fade => IDC_FADE,
                    Field::Latitude => IDC_LATITUDE,
                    Field::Longitude => IDC_LONGITUDE,
                    Field::Monitor => IDC_MONITOR,
                };
                let editor = control::item(self.page(field.page()), id);
                // Scrolling need not move focus. Reveal explicitly even if
                // this same editor already owns focus and gets no new message.
                control::reveal(self.page(field.page()), editor);
                if matches!(field, Field::Detail | Field::Monitor) {
                    // SAFETY: Explicit validation focus on a live native combo.
                    unsafe {
                        let _ = SetFocus(Some(editor));
                    }
                } else {
                    control::select_edit(editor);
                    let title = HSTRING::from("Invalid value");
                    let reason = self.model.borrow().invalid.as_ref().unwrap().reason;
                    let reason = HSTRING::from(reason);
                    let tip = EDITBALLOONTIP {
                        cbStruct: std::mem::size_of::<EDITBALLOONTIP>() as u32,
                        pszTitle: windows::core::PCWSTR(title.as_ptr()),
                        pszText: windows::core::PCWSTR(reason.as_ptr()),
                        ttiIcon: TTI_ERROR,
                    };
                    // SAFETY: The edit copies the bounded validation strings
                    // during this explicit UI-thread notification.
                    unsafe {
                        SendMessageW(
                            editor,
                            EM_SHOWBALLOONTIP,
                            None,
                            Some(LPARAM(&tip as *const _ as isize)),
                        );
                    }
                }
            }
        }
    }

    fn command(&self, id: u16, code: u16, source: HWND) {
        match id {
            id if id == IDCANCEL.0 as u16 => {
                // SAFETY: Accepted saves/operations remain application-owned.
                unsafe {
                    let _ = EndDialog(self.window.get(), 0);
                }
            }
            id if id == IDOK.0 as u16 => self.accept(Intent::Ok),
            IDC_APPLY => self.accept(Intent::Apply),
            IDC_LOCATION_SETTINGS => self.select_page(Page::Location),
            IDC_SEARCH if code == EN_CHANGE as u16 && !self.filling.get() => self.filter(),
            IDC_CLEAR => {
                control::set_text(self.item(IDC_SEARCH), "");
                control::select_edit(self.item(IDC_SEARCH));
            }
            IDC_GET_CITY => self.app.download_city(),
            IDC_CANCEL_CITY_DATA => self.app.cancel_city(),
            IDC_INSTALL_SAVER => self.app.screen_saver_action(SaverAction::Install),
            IDC_SAVER_WINDOWS => self.app.screen_saver_action(SaverAction::WindowsSettings),
            IDC_PREVIEW_SAVER => self.app.screen_saver_action(SaverAction::Preview),
            IDC_RESET_PAGE if self.can_edit() => {
                if let Some(page) = self.shown.get() {
                    self.model.borrow_mut().draft.reset(page);
                    self.model.borrow_mut().invalid = None;
                    self.fill_page(page);
                    self.refresh();
                }
            }
            IDC_RESET_ALL if self.can_edit() => {
                // SAFETY: No model borrow crosses this nested native modal loop.
                let yes = unsafe {
                    MessageBoxW(
                        Some(self.window.get()),
                        &HSTRING::from(
                            "Restore all editable preferences to defaults? Choose Apply or OK to save.",
                        ),
                        &HSTRING::from("Reset all settings"),
                        MB_YESNO | MB_DEFBUTTON2 | MB_ICONQUESTION,
                    )
                };
                if yes == IDYES && self.can_edit() {
                    {
                        let mut model = self.model.borrow_mut();
                        model.draft = Draft::from(&Preferences::default());
                        model.invalid = None;
                    }
                    for page in Page::ALL {
                        self.fill_page(page);
                    }
                    self.refresh();
                }
            }
            _ if code == EN_CHANGE as u16
                || code == CBN_SELCHANGE as u16
                || code == BN_CLICKED as u16 =>
            {
                self.changed(source, id)
            }
            _ => {}
        }
    }

    fn select_page(&self, page: Page) {
        {
            let _fill = Fill::new(&self.filling);
            control::set_text(self.item(IDC_SEARCH), "");
        }
        self.shown.set(Some(page));
        self.filter();
    }

    fn layout(&self) {
        let window = self.window.get();
        if window.is_invalid() {
            return;
        }
        // SAFETY: All coordinates and structures are local and all HWNDs are
        // children of this live shell. Resize does not recreate any page.
        unsafe {
            let mut client = RECT::default();
            if GetClientRect(window, &mut client).is_err() {
                return;
            }
            let x = |du| native::units(window, du, 0, 0, 0).left;
            let y = |du| native::units(window, 0, du, 0, 0).top;
            let width = client.right;
            let height = client.bottom;
            let search_label = self.fonts.get().map_or(x(70), |fonts| {
                native::text_width(
                    window,
                    fonts.normal,
                    &[&control::text(self.item(IDC_SEARCH_LABEL))],
                )
            });
            let search_left = x(14) + search_label;
            let navigation = self.navigation_width.get().max(x(80));
            let compact = width < x(280);
            let reset_above = width < x(255);
            let page_left = if compact { x(7) } else { x(14) + navigation };
            let page_top = y(if compact { 82 } else { 28 });
            let footer = height - y(32);
            let page_bottom = footer - y(26);
            let rect = |left, top, width, height| RECT {
                left,
                top,
                right: left + width,
                bottom: top + height,
            };
            let mut positions = vec![
                (
                    self.item(IDC_SEARCH_LABEL),
                    rect(x(7), y(9), search_label, y(10)),
                ),
                (
                    self.item(IDC_SEARCH),
                    rect(search_left, y(7), width - x(68) - search_left, y(14)),
                ),
                (
                    self.item(IDC_CLEAR),
                    rect(width - x(61), y(7), x(54), y(14)),
                ),
                (
                    self.item(IDC_PAGES_LABEL),
                    rect(
                        x(7),
                        y(28),
                        if compact { width - x(14) } else { navigation },
                        y(10),
                    ),
                ),
                (
                    self.item(IDC_PAGES),
                    rect(
                        x(7),
                        y(42),
                        if compact { width - x(14) } else { navigation },
                        if compact { y(32) } else { footer - y(50) },
                    ),
                ),
                (
                    self.item(IDC_NO_MATCH),
                    rect(page_left, page_top + y(2), width - page_left - x(7), y(28)),
                ),
                (
                    self.item(IDC_RESET_PAGE),
                    rect(width - x(61), footer - y(22), x(54), y(14)),
                ),
                (
                    self.item(IDC_RESET_ALL),
                    rect(
                        x(7),
                        footer - if reset_above { y(22) } else { 0 },
                        x(65),
                        y(14),
                    ),
                ),
                (
                    self.item(IDOK.0 as u16),
                    rect(width - x(183), footer, x(54), y(14)),
                ),
                (
                    self.item(IDCANCEL.0 as u16),
                    rect(width - x(122), footer, x(54), y(14)),
                ),
                (
                    self.item(IDC_APPLY),
                    rect(width - x(61), footer, x(54), y(14)),
                ),
                (
                    self.item(IDC_SETTINGS_STATUS),
                    rect(0, height - y(13), width, y(13)),
                ),
            ];
            for page in Page::ALL {
                let child = self.page(page);
                if !child.is_invalid() {
                    positions.push((
                        child,
                        rect(
                            page_left,
                            page_top,
                            width - page_left - x(7),
                            (page_bottom - page_top).max(1),
                        ),
                    ));
                }
            }
            control::place(window, &positions);
            let parts = [x(135), -1];
            SendMessageW(
                self.item(IDC_SETTINGS_STATUS),
                SB_SETPARTS,
                Some(WPARAM(parts.len())),
                Some(LPARAM(parts.as_ptr() as isize)),
            );
            let mut list_client = RECT::default();
            let _ = GetClientRect(self.item(IDC_PAGES), &mut list_client);
            let column = (list_client.right - 4).max(0);
            if SendMessageW(
                self.item(IDC_PAGES),
                LVM_GETCOLUMNWIDTH,
                Some(WPARAM(0)),
                None,
            )
            .0 != column as isize
            {
                SendMessageW(
                    self.item(IDC_PAGES),
                    LVM_SETCOLUMNWIDTH,
                    Some(WPARAM(0)),
                    Some(LPARAM(column as isize)),
                );
            }
        }
        self.layout_pages();
    }
}

impl Drop for Dialog<'_> {
    fn drop(&mut self) {
        if let Some(fonts) = self.fonts.take() {
            fonts.release();
        }
    }
}

unsafe extern "system" fn set_font(window: HWND, font: LPARAM) -> BOOL {
    native::font(
        window,
        windows::Win32::Graphics::Gdi::HFONT(font.0 as *mut _),
    );
    BOOL(1)
}

unsafe extern "system" fn install_keys(window: HWND, context: LPARAM) -> BOOL {
    // SAFETY: Each control removes its subclass at NCDESTROY, before the
    // shell's stable feature pointer can leave the native modal loop.
    unsafe {
        let _ = SetWindowSubclass(window, Some(keys_proc), 1, context.0 as usize);
    }
    BOOL(1)
}

unsafe extern "system" fn keys_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    context: usize,
) -> LRESULT {
    // SAFETY: Installed only on this feature's descendants, retired at destroy.
    let dialog = unsafe { &*(context as *const Dialog) };
    if message == WM_SETFOCUS {
        for page in Page::ALL {
            let parent = dialog.page(page);
            // SAFETY: Only page descendants receive this scoped reveal effect.
            if !parent.is_invalid() && unsafe { IsChild(parent, window).as_bool() } {
                control::reveal(parent, window);
                break;
            }
        }
    }
    if message == WM_KEYDOWN {
        // SAFETY: Thread-local key state and focus for this live native dialog.
        unsafe {
            if wparam.0 == VK_F.0 as usize && GetKeyState(VK_CONTROL.0 as i32) < 0 {
                control::select_edit(dialog.item(IDC_SEARCH));
                return LRESULT(0);
            }
            if wparam.0 == VK_RETURN.0 as usize && window == dialog.item(IDC_SEARCH) {
                if dialog.shown.get().is_some() {
                    let _ = SetFocus(Some(dialog.item(IDC_PAGES)));
                }
                return LRESULT(0);
            }
        }
    }
    if message == WM_GETDLGCODE
        && window == dialog.item(IDC_SEARCH)
        && wparam.0 == VK_RETURN.0 as usize
    {
        return LRESULT(DLGC_WANTALLKEYS as isize);
    }
    // SAFETY: Continue the standard control chain; removal precedes final
    // forwarding and never frees the modal feature allocation.
    unsafe {
        if message == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(window, Some(keys_proc), 1);
        }
        DefSubclassProc(window, message, wparam, lparam)
    }
}

unsafe extern "system" fn shell_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    // SAFETY: WM_INITDIALOG supplies Box-owned feature state, retained until
    // DialogBoxParam returns. NCDESTROY clears the pointer and registration.
    unsafe {
        if message == WM_INITDIALOG {
            SetWindowLongPtrW(window, GWLP_USERDATA, lparam.0);
        }
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *const Dialog;
        let Some(dialog) = pointer.as_ref() else {
            return 0;
        };
        match message {
            WM_INITDIALOG => {
                dialog.window.set(window);
                let _ =
                    EnumChildWindows(Some(window), Some(install_keys), LPARAM(pointer as isize));
                dialog.initialize();
                return 0;
            }
            WM_COMMAND => {
                dialog.command(
                    wparam.0 as u16,
                    (wparam.0 >> 16) as u16,
                    HWND(lparam.0 as *mut _),
                );
                return 1;
            }
            WM_NOTIFY => {
                let notification = &*(lparam.0 as *const NMHDR);
                if notification.idFrom == IDC_PAGES as usize
                    && notification.code == LVN_ITEMCHANGED
                    && !dialog.filling.get()
                {
                    let selected = SendMessageW(
                        dialog.item(IDC_PAGES),
                        LVM_GETNEXTITEM,
                        Some(WPARAM(usize::MAX)),
                        Some(LPARAM(LVNI_SELECTED as isize)),
                    )
                    .0;
                    let page = dialog.listed.borrow().get(selected as usize).copied();
                    if let Some(page) = page {
                        dialog.shown.set(Some(page));
                        dialog.last.set(page);
                        dialog.filter();
                    }
                }
                return 0;
            }
            REFRESH if wparam.0 == dialog.id as usize => {
                dialog.refresh();
                return 1;
            }
            SELECT_PAGE if wparam.0 == dialog.id as usize => {
                if let Some(page) = Page::ALL.get(lparam.0 as usize) {
                    dialog.select_page(*page);
                }
                return 1;
            }
            WM_SIZE => {
                dialog.layout();
                return 1;
            }
            WM_GETMINMAXINFO => {
                let limits = &mut *(lparam.0 as *mut MINMAXINFO);
                let minimum = native::minimum(window, 280, 220);
                limits.ptMinTrackSize.x = minimum.cx;
                limits.ptMinTrackSize.y = minimum.cy;
                return 1;
            }
            WM_DPICHANGED => {
                dialog.update_fonts();
                native::fit_work_area(window, Some(*(lparam.0 as *const RECT)));
                dialog.layout();
                return 1;
            }
            WM_SETTINGCHANGE | WM_DISPLAYCHANGE => {
                dialog.update_fonts();
                native::fit_work_area(window, None);
                dialog.layout();
                return 0;
            }
            WM_CLOSE => {
                let _ = EndDialog(window, 0);
                return 1;
            }
            WM_NCDESTROY => {
                if dialog
                    .app
                    .settings_dialog
                    .get()
                    .is_some_and(|(id, live)| id == dialog.id && live == window)
                {
                    dialog.app.settings_dialog.set(None);
                }
                dialog.window.set(HWND::default());
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            }
            _ => {}
        }
        0
    }
}

unsafe extern "system" fn page_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    // SAFETY: Page state borrows the same modal-lifetime allocation as shell.
    unsafe {
        match message {
            WM_INITDIALOG => {
                SetWindowLongPtrW(window, GWLP_USERDATA, lparam.0);
                return 1;
            }
            WM_COMMAND => {
                if let Ok(parent) = GetParent(window) {
                    SendMessageW(parent, message, Some(wparam), Some(lparam));
                }
                return 1;
            }
            WM_NOTIFY => {
                let header = &*(lparam.0 as *const NMHDR);
                if header.code == UDN_DELTAPOS {
                    let spin = &*(lparam.0 as *const NMUPDOWN);
                    let id = if header.idFrom == IDC_BRIGHT_SPIN as usize {
                        IDC_BRIGHT
                    } else {
                        IDC_FADE
                    };
                    let editor = control::item(window, id);
                    if let Ok(value) = control::text(editor).trim().parse::<u64>()
                        && let Some(value) = value.checked_add_signed(i64::from(spin.iDelta))
                    {
                        control::set_text(editor, &value.to_string());
                    }
                    SetWindowLongPtrW(window, WINDOW_LONG_PTR_INDEX(DWLP_MSGRESULT as i32), 1);
                    return 1;
                }
            }
            WM_VSCROLL => {
                let mut info = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_ALL,
                    ..Default::default()
                };
                if GetScrollInfo(window, SB_VERT, &mut info).is_ok() {
                    let previous = info.nPos;
                    let line = native::units(window, 0, 0, 0, 12).bottom;
                    info.nPos = match wparam.0 as u16 as u32 {
                        0 => previous - line,
                        1 => previous + line,
                        2 => previous - info.nPage as i32,
                        3 => previous + info.nPage as i32,
                        4 | 5 => info.nTrackPos,
                        6 => info.nMin,
                        7 => info.nMax,
                        _ => previous,
                    };
                    control::scroll(window, info.nPos);
                }
                return 1;
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            }
            _ => {}
        }
        0
    }
}

//! In-process native control journey. Never compiled into NetMap releases.

use super::*;
use crate::{
    app::native_checks as facts,
    connections::{Life, ObservationId},
    map::model::{Model, Place},
};
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        System::SystemServices::MK_LBUTTON,
        UI::{Controls::*, Input::KeyboardAndMouse::VK_ESCAPE},
    },
    core::PWSTR,
};
use windows_numerics::Vector2;

thread_local! { static REQUESTED: Cell<bool> = const { Cell::new(false) }; }

pub(super) fn requested() -> bool {
    REQUESTED.get()
}

#[test]
#[ignore = "Creates only owned native map/ListView windows with deterministic accepted facts; run alone with a process watchdog"]
fn native_accepted_observations_preserve_group_and_row_identity() -> Result<()> {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            REQUESTED.set(false);
        }
    }
    REQUESTED.set(true);
    let _scope = Scope;
    facts::run()
}

fn require(value: bool, reason: &str) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(Error::new(E_FAIL, reason))
    }
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let result = journey(host);
    // SAFETY: This fixture owns the main HWND on its creating thread. Cleanup
    // runs outside every FFI callback, for both success and returned failures.
    let cleanup = unsafe { DestroyWindow(host.window.get()) };
    result.and(cleanup)
}

fn pump(host: &Host) -> Result<()> {
    let mut message = MSG::default();
    for _ in 0..512 {
        // SAFETY: Same UI thread and ordinary production dispatch order. No
        // other application is addressed and no physical input is injected.
        unsafe {
            if !PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                return Ok(());
            }
            if message.message == WM_QUIT {
                PostQuitMessage(message.wParam.0 as i32);
                return Err(Error::new(
                    E_FAIL,
                    "Fixture window retired before its checks completed",
                ));
            }
            if host.navigation_key(&message)
                || IsDialogMessageW(host.window.get(), &message).as_bool()
            {
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Err(Error::new(
        E_FAIL,
        "Native fixture message dispatch did not quiesce within its bound",
    ))
}

fn command(host: &Host, id: u16) -> Result<()> {
    // SAFETY: Numeric command to this same-thread owned native window.
    unsafe {
        SendMessageW(
            host.window.get(),
            WM_COMMAND,
            Some(WPARAM(id as usize)),
            None,
        );
    }
    pump(host)
}

fn table(host: &Host) -> Result<&crate::app::exploration::Exploration> {
    host.exploration
        .get()
        .ok_or_else(|| Error::new(E_FAIL, "Native exploration control missing"))
}

fn text(host: &Host, row: usize, column: usize) -> Result<String> {
    let window = table(host)?.window();
    let mut buffer = [0u16; 512];
    let mut item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: row as i32,
        iSubItem: column as i32,
        pszText: PWSTR(buffer.as_mut_ptr()),
        cchTextMax: buffer.len() as i32,
        ..Default::default()
    };
    // SAFETY: Both the LVITEMW and text buffer are in this same process and
    // remain live through the synchronous real ListView GETDISPINFO request.
    let available = unsafe {
        SendMessageW(
            window,
            LVM_GETITEMW,
            None,
            Some(LPARAM(&mut item as *mut _ as isize)),
        )
    }
    .0;
    require(available != 0, "Native virtual item request failed")?;
    let length = buffer
        .iter()
        .position(|&unit| unit == 0)
        .ok_or_else(|| Error::new(E_FAIL, "Native cell was not terminated within its bound"))?;
    Ok(String::from_utf16_lossy(&buffer[..length]))
}

fn expect_rows(host: &Host, ids: &[u64], cities: bool) -> Result<()> {
    // SAFETY: Numeric query of this UI thread's own virtual ListView.
    let count = unsafe { SendMessageW(table(host)?.window(), LVM_GETITEMCOUNT, None, None) }.0;
    require(
        count == ids.len() as isize,
        "Displayed row count disagrees with accepted membership",
    )?;
    let original = facts::observations();
    for (row, &id) in ids.iter().enumerate() {
        let observation = original
            .iter()
            .find(|observation| observation.id == ObservationId(id))
            .ok_or_else(|| Error::new(E_FAIL, "Unknown declared fixture identity"))?;
        let cells: Vec<_> = (0..9)
            .map(|column| text(host, row, column))
            .collect::<Result<_>>()?;
        require(
            cells[0] == format!("fixture-{}.exe", observation.tuple.process.pid),
            "Ready process metadata changed",
        )?;
        require(
            cells[1] == observation.tuple.process.pid.to_string() && cells[2] == "TCP",
            "Native PID/protocol disagrees with accepted observation",
        )?;
        require(
            cells[3]
                == format!(
                    "{}:{}",
                    observation.tuple.local.address(),
                    observation.tuple.local.port()
                )
                && cells[4]
                    == format!(
                        "{}:{}",
                        observation.tuple.remote.address(),
                        observation.tuple.remote.port()
                    ),
            "Native endpoints disagree with accepted observation",
        )?;
        require(
            cells[5]
                == if matches!(observation.life, Life::EndedTcp) {
                    "Ended"
                } else {
                    "Established"
                },
            "Native lifetime text disagrees with accepted observation",
        )?;
        require(
            cells[7] == "Unavailable" && cells[8] == "Unavailable",
            "Unknown traffic was presented as measured",
        )?;
        let precision = match id {
            13 => "loopback",
            14 => "current dataset",
            12 if cities => "Approximate coordinate",
            _ => "Country estimate",
        };
        require(
            cells[6].contains(precision),
            "Native row precision/unplottable reason is inaccurate",
        )?;
        eprintln!("native-exploration row={row} observation={id} cells={cells:?}");
    }
    Ok(())
}

fn select(host: &Host, index: usize, id: u64) -> Result<()> {
    let item = LVITEMW {
        stateMask: LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0),
        state: LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0),
        ..Default::default()
    };
    // SAFETY: In-process synchronous item state update. The native control sends
    // its real ITEMCHANGED notification through the production Host procedure.
    unsafe {
        SendMessageW(
            table(host)?.window(),
            LVM_SETITEMSTATE,
            Some(WPARAM(index)),
            Some(LPARAM(&item as *const _ as isize)),
        );
    }
    pump(host)?;
    require(
        table(host)?.selected() == Some(ObservationId(id)),
        "Native selection did not resolve to its exact observation",
    )?;
    require(
        host.app.selected_place()
            == host
                .app
                .exploration_rows()
                .and_then(|rows| rows.place(ObservationId(id))),
        "Row-to-map selection disagrees with the accepted row location",
    )
}

fn click(host: &Host, point: Vector2) -> Result<()> {
    // SAFETY: Query of the same-thread owned map. Convert only its DIP point.
    let dpi = unsafe { GetDpiForWindow(host.map_window.get()) }.max(96) as f32;
    let (x, y) = (
        (point.X * dpi / 96.0).round() as i16,
        (point.Y * dpi / 96.0).round() as i16,
    );
    let position = LPARAM((x as u16 as usize | ((y as u16 as usize) << 16)) as isize);
    // SAFETY: Mouse messages target only this fixture's native map; its actual
    // capture/navigation/click interpretation runs without global input.
    unsafe {
        SendMessageW(
            host.map_window.get(),
            WM_LBUTTONDOWN,
            Some(WPARAM(MK_LBUTTON.0 as usize)),
            Some(position),
        );
        SendMessageW(host.map_window.get(), WM_LBUTTONUP, None, Some(position));
    }
    pump(host)
}

fn destination(
    model: &Model,
    cities: bool,
    estimated: bool,
) -> Result<&crate::map::model::Destination> {
    model
        .destinations
        .iter()
        .find(|destination| {
            let group = &model.groups[destination.group];
            group.place.cities == cities && group.estimated == estimated
        })
        .ok_or_else(|| Error::new(E_FAIL, "Authoritative fixture destination missing"))
}

fn settle(host: &Host, model: &Model) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        pump(host)?;
        host.paint(host.map_window.get());
        let view = host.app.map.viewport();
        if model
            .destinations
            .iter()
            .all(|destination| host.app.map.hit(view.screen(destination.point)).is_some())
        {
            return Ok(());
        }
        require(
            Instant::now() < deadline,
            "Native map did not prepare fixture hit testing",
        )?;
    }
}

fn guide_point(
    host: &Host,
    model: &Model,
    target: &crate::map::model::Destination,
) -> Result<Vector2> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        host.paint(host.map_window.get());
        let view = host.app.map.viewport();
        for y in (12..view.size.Y.max(12.0) as usize).step_by(6) {
            require(
                Instant::now() < deadline,
                "No prepared native guide point found within fixture bound",
            )?;
            for x in (12..view.size.X.max(12.0) as usize).step_by(6) {
                let point = Vector2::new(x as f32, y as f32);
                if model.destinations.iter().any(|destination| {
                    (view.screen(destination.point) - point).length_squared() < 400.0
                }) {
                    continue;
                }
                // Query the production hit tester; this fixture contains no
                // guide/projection reconstruction or alternate hit algorithm.
                if [
                    Vector2::zero(),
                    Vector2::new(-1.0, -1.0),
                    Vector2::new(-1.0, 1.0),
                    Vector2::new(1.0, -1.0),
                    Vector2::new(1.0, 1.0),
                ]
                .iter()
                .all(|offset| host.app.map.hit(point + *offset).as_ref() == Some(&target.key))
                {
                    return Ok(point);
                }
            }
        }
        pump(host)?;
    }
}

fn expect_filter(host: &Host, place: &Place, members: &[u64], cities: bool) -> Result<()> {
    require(
        host.app.exploration_filter().as_ref() == Some(place)
            && host.app.selected_place().as_ref() == Some(place),
        "Map click did not commit the authoritative place/filter",
    )?;
    expect_rows(host, members, cities)
}

fn journey(host: &Host) -> Result<()> {
    // SAFETY: Explicitly show only the owned fixture window; two calls handle
    // a hidden STARTUPINFO launch without foreground or physical input changes.
    unsafe {
        let _ = ShowWindow(host.window.get(), SW_SHOWNA);
        let _ = ShowWindow(host.window.get(), SW_SHOWNA);
    }
    command(host, ids::IDM_MAP_TABLE)?;
    let snapshot = facts::facts(&host.app, facts::observations(), 100, 1, None);
    let model = facts::scene(&host.app, &snapshot, false);
    pump(host)?;
    expect_rows(host, &[10, 11, 12, 13, 14], false)?;
    settle(host, &model)?;
    let country = destination(&model, false, true)?;
    let place = model.groups[country.group].place.clone();
    let members: Vec<_> = model.groups[country.group]
        .observations
        .iter()
        .map(|id| id.0)
        .collect();
    require(
        members == [10, 11, 12],
        "Country fixture membership changed",
    )?;
    click(host, host.app.map.viewport().screen(country.point))?;
    expect_filter(host, &place, &members, false)?;
    command(host, ids::IDM_ALL_CONNECTIONS)?;
    settle(host, &model)?;
    click(host, guide_point(host, &model, country)?)?;
    expect_filter(host, &place, &members, false)?;
    eprintln!(
        "native-exploration check=country-marker-and-guide place={place:?} members={members:?}"
    );

    select(host, 0, 10)?;
    let previous = host.app.exploration_rows().unwrap();
    let pending = facts::facts(&host.app, facts::observations(), 102, 1, Some(&previous));
    pump(host)?;
    require(
        table(host)?.selected() == Some(ObservationId(10)),
        "Pending lookup moved an ended row's selected identity",
    )?;
    require(
        host.app.exploration_filter().as_ref() == Some(&place),
        "Pending lookup discarded the selected country filter",
    )?;
    // SAFETY: Numeric query of the same-thread owned native list.
    let pending_count =
        unsafe { SendMessageW(table(host)?.window(), LVM_GETITEMCOUNT, None, None) }.0;
    require(
        pending_count == 3,
        "Pending lookup lost known surviving filtered membership",
    )?;
    require(
        text(host, 0, 6)?.contains("updating"),
        "Pending lookup retained a false ready location caption",
    )?;
    let model = facts::scene(&host.app, &pending, true);
    pump(host)?;
    require(
        table(host)?.selected() == Some(ObservationId(10)),
        "Ready scene moved pending row selection",
    )?;
    require(
        host.app.selected_place()
            == host
                .app
                .exploration_rows()
                .and_then(|rows| rows.place(ObservationId(10))),
        "Ready detail change did not reconcile selected row's place",
    )?;
    command(host, ids::IDM_ALL_CONNECTIONS)?;
    settle(host, &model)?;
    let coordinate = destination(&model, true, false)?;
    let coordinate_place = model.groups[coordinate.group].place.clone();
    click(host, host.app.map.viewport().screen(coordinate.point))?;
    expect_filter(host, &coordinate_place, &[12], true)?;
    select(host, 0, 12)?;
    let view = host.app.map.viewport();
    // SAFETY: Queued key traverses the production navigation/dialog message path.
    unsafe {
        PostMessageW(
            Some(table(host)?.window()),
            WM_KEYDOWN,
            WPARAM(VK_ESCAPE.0 as usize),
            LPARAM(0),
        )?;
    }
    pump(host)?;
    require(
        host.app.exploration_filter().is_none() && table(host)?.selected().is_none(),
        "Escape did not restore All and clear exact row selection",
    )?;
    require(
        host.app.map.viewport() == view,
        "Clearing exploration moved the viewport",
    )?;
    expect_rows(host, &[10, 11, 12, 13, 14], true)?;
    eprintln!("native-exploration check=pending-coordinate-filter-escape");

    select(host, 0, 10)?;
    let previous = host.app.exploration_rows().unwrap();
    let survivors: Vec<_> = facts::observations()
        .into_iter()
        .filter(|observation| observation.id != ObservationId(10))
        .collect();
    let snapshot = facts::facts(&host.app, survivors, 135, 1, Some(&previous));
    facts::scene(&host.app, &snapshot, true);
    pump(host)?;
    require(
        table(host)?.selected().is_none(),
        "Retired ended A selected same-tuple successor B",
    )?;
    require(
        host.app.selected_place().is_some(),
        "Retiring one row discarded a place still represented by B",
    )?;
    expect_rows(host, &[11, 12, 13, 14], true)?;
    select(host, 0, 11)?;
    command(host, ids::IDM_MAP_ONLY)?;
    command(host, ids::IDM_MAP_TABLE)?;
    require(
        table(host)?.selected() == Some(ObservationId(11)),
        "Map/Both view changes lost the surviving idle row",
    )?;
    eprintln!(
        "native-exploration check=ended-A-retired-live-same-tuple-B-preserved at=135 selected=11"
    );

    let mut reset = facts::observations();
    for observation in &mut reset {
        observation.tuple.process.generation = 2;
        observation.introduced = Duration::from_secs(136);
    }
    let snapshot = facts::facts(&host.app, reset, 136, 2, None);
    facts::scene(&host.app, &snapshot, true);
    pump(host)?;
    require(
        table(host)?.selected().is_none()
            && host.app.selected_place().is_none()
            && host.app.exploration_filter().is_none(),
        "Source reset inherited a reused observation ID's selection",
    )?;
    expect_rows(host, &[10, 11, 12, 13, 14], true)?;
    eprintln!("native-exploration check=source-reset-reused-ids-clear");

    select(host, 1, 11)?;
    resize(host, 320, 260)?;
    // SAFETY: Native scrolling operates on this fixture's own ListView.
    let top = unsafe {
        SendMessageW(
            table(host)?.window(),
            LVM_ENSUREVISIBLE,
            Some(WPARAM(4)),
            None,
        );
        SendMessageW(table(host)?.window(), LVM_GETTOPINDEX, None, None).0
    };
    require(
        top > 0,
        "Compact fixture did not exercise native list scrolling",
    )?;
    let previous = host.app.exploration_rows().unwrap();
    let snapshot = facts::facts(
        &host.app,
        snapshot.observations.to_vec(),
        138,
        2,
        Some(&previous),
    );
    facts::scene(&host.app, &snapshot, true);
    pump(host)?;
    // SAFETY: Read only this native control's interaction-owned scroll state.
    let current_top = unsafe { SendMessageW(table(host)?.window(), LVM_GETTOPINDEX, None, None) }.0;
    require(current_top == top, "Current facts reset native list scroll")?;
    require(
        table(host)?.selected() == Some(ObservationId(11)),
        "Current facts lost the surviving selected identity",
    )?;
    resize(host, 1300, 550)?;
    require(
        !host.stacked.get(),
        "Wide Both view did not choose side arrangement",
    )?;
    command(host, ids::IDM_MORE_TABLE)?;
    require(
        (host.side_fraction.get() - 0.60).abs() < 0.0001,
        "More table did not change side proportion",
    )?;
    resize(host, 650, 1100)?;
    require(
        host.stacked.get(),
        "Tall Both view did not choose stacked arrangement",
    )?;
    command(host, ids::IDM_MORE_MAP)?;
    require(
        (host.stacked_fraction.get() - 0.45).abs() < 0.0001,
        "More map did not change stacked proportion",
    )?;
    resize(host, 1300, 550)?;
    require(
        !host.stacked.get() && (host.side_fraction.get() - 0.60).abs() < 0.0001,
        "Switching orientation lost the side proportion",
    )?;
    resize(host, 650, 1100)?;
    require(
        host.stacked.get() && (host.stacked_fraction.get() - 0.45).abs() < 0.0001,
        "Switching orientation lost the stacked proportion",
    )?;
    require(
        table(host)?.selected() == Some(ObservationId(11)),
        "Layout commands lost native selected identity",
    )?;
    eprintln!(
        "native-exploration check=both-orientations-independent-proportions-and-scroll top={top} selected=11"
    );
    Ok(())
}

fn resize(host: &Host, width: i32, height: i32) -> Result<()> {
    // SAFETY: Resize only the fixture's own top-level HWND; production WM_SIZE
    // computes fitted arrangement and positions every native child.
    unsafe {
        SetWindowPos(
            host.window.get(),
            None,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
    }
    pump(host)
}

//! Real ordinary-token source acceptance; the external wrapper owns elevation.

use super::*;
use crate::{
    app::work::Source,
    connections::{Coverage, Life, TcpState},
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Mutex, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
        System::Threading::{OpenEventW, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject},
        UI::Controls::{
            LVIF_STATE, LVIF_TEXT, LVIS_FOCUSED, LVIS_SELECTED, LVITEMW, LVM_GETITEMW,
            LVM_SETITEMSTATE,
        },
    },
    core::{GUID, PWSTR},
};

const TIMER: usize = 0x4e42;
thread_local! { static REQUESTED: Cell<bool> = const { Cell::new(false) }; }

#[derive(Default)]
struct Sequences {
    epochs: Vec<(GUID, u64, u64, u32)>,
    invalid: bool,
}
static SEQUENCES: Mutex<Option<Sequences>> = Mutex::new(None);

pub(crate) fn received(epoch: GUID, sequence: u64) {
    let Ok(mut scoped) = SEQUENCES.lock() else {
        return;
    };
    let Some(state) = scoped.as_mut() else { return };
    if let Some((_, _, previous, count)) = state.epochs.iter_mut().find(|row| row.0 == epoch) {
        state.invalid |= sequence <= *previous;
        *previous = sequence;
        *count += 1;
    } else if state.epochs.len() < 2 {
        state.epochs.push((epoch, sequence, sequence, 1));
    } else {
        state.invalid = true;
    }
}

pub(super) fn requested() -> bool {
    REQUESTED.get()
}

#[test]
#[ignore = "Requires the reviewed ordinary-token broker wrapper and its owned elevated production collectors"]
fn native_elevated_broker_sources_and_viewer_rows() -> Result<()> {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            REQUESTED.set(false);
            *SEQUENCES.lock().unwrap() = None;
        }
    }
    require(
        std::env::var_os("NETMAP_BROKER_EVENTS").is_some(),
        "Use tests/native-broker.ps1",
    )?;
    REQUESTED.set(true);
    *SEQUENCES.lock().unwrap() = Some(Sequences::default());
    let _scope = Scope;
    crate::app::native_checks::run()
}

struct Event(HANDLE);
impl Event {
    fn open(name: &str) -> Result<Self> {
        let name = HSTRING::from(name);
        // SAFETY: The wrapper owns this uniquely named event. This handle may
        // only wait; no foreign event is signalled or modified by the fixture.
        unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, false, &name).map(Self) }
    }
    fn ready(&self) -> bool {
        // SAFETY: Owned live event handle, nonblocking readiness query.
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 }
    }
}
impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: Exactly this OpenEvent handle is closed once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub(super) fn exercise(host: &Host) -> Result<()> {
    let result = journey(host);
    // SAFETY: Cleanup occurs outside FFI on this Host's creating thread.
    let cleanup = unsafe {
        let _ = KillTimer(Some(host.window.get()), TIMER);
        DestroyWindow(host.window.get())
    };
    result.and(cleanup)
}

fn journey(host: &Host) -> Result<()> {
    let prefix = std::env::var("NETMAP_BROKER_EVENTS").map_err(|e| failure(&e.to_string()))?;
    let pause = Event::open(&format!("{prefix}-pause"))?;
    let resume = Event::open(&format!("{prefix}-resume"))?;
    let finish = Event::open(&format!("{prefix}-finish"))?;
    // SAFETY: Fixture-local timer bounds message waits; show only its own HWND.
    // The second call overrides Hidden STARTUPINFO on the first native show.
    unsafe {
        require(
            SetTimer(Some(host.window.get()), TIMER, 20, None) != 0,
            "Broker fixture timer failed",
        )?;
        let _ = ShowWindow(host.window.get(), SW_SHOWNA);
        let _ = ShowWindow(host.window.get(), SW_SHOWNA);
    }
    require(
        host.exploration_command(ids::IDM_TABLE_ONLY),
        "Table command unavailable",
    )?;
    host.app
        .native_broker_resume()
        .map_err(|e| failure(&e.to_string()))?;
    let Traffic {
        local,
        remote,
        finished: traffic,
    } = traffic(host)?;
    let mut first = None;
    wait(
        host,
        25,
        || {
            let Some((Source::Broker(epoch), publication)) = host.app.native_broker_publication()
            else {
                return Ok(false);
            };
            if publication.snapshot.coverage != Coverage::Complete {
                return Ok(false);
            }
            let Some((index, row)) =
                publication
                    .snapshot
                    .observations
                    .iter()
                    .enumerate()
                    .find(|(_, row)| {
                        row.tuple.process.pid == std::process::id()
                            && row.tuple.local == local
                            && row.tuple.remote == remote
                            && row.life == Life::Tcp(TcpState::Established)
                            && row.bytes.received > 0
                            && row.bytes.sent > 0
                            && row
                                .rates
                                .is_some_and(|rates| rates.received > 0.0 && rates.sent > 0.0)
                    })
            else {
                return Ok(false);
            };
            host.refresh_exploration();
            let table = table(host)?;
            let cells = (0..9)
                .map(|column| text(table.window(), index, column))
                .collect::<Result<Vec<_>>>()?;
            require(
                cells[1] == std::process::id().to_string() && cells[2] == "TCP",
                "Native PID/protocol mismatch",
            )?;
            require(
                cells[3] == format!("{}:{}", local.address(), local.port())
                    && cells[4] == format!("{}:{}", remote.address(), remote.port()),
                "Native endpoints mismatch",
            )?;
            require(
                cells[6].contains("loopback"),
                "Native loopback reason unavailable",
            )?;
            let rates = row.rates.unwrap();
            require(
                cells[7] == format!("{:.0} B/s", rates.received)
                    && cells[8] == format!("{:.0} B/s", rates.sent),
                "Native measured rates differ from accepted broker facts",
            )?;
            let name = publication
                .names
                .get(&row.tuple.process)
                .ok_or_else(|| failure("Broker process instance name unavailable"))?;
            require(
                cells[0] == name.as_ref() && !cells[0].is_empty(),
                "Native process name differs from accepted process instance",
            )?;
            require(
                cells[5] == "Established",
                "Native state differs from accepted established TCP connection",
            )?;
            let item = LVITEMW {
                mask: LVIF_STATE,
                state: windows::Win32::UI::Controls::LIST_VIEW_ITEM_STATE_FLAGS(
                    LVIS_SELECTED.0 | LVIS_FOCUSED.0,
                ),
                stateMask: windows::Win32::UI::Controls::LIST_VIEW_ITEM_STATE_FLAGS(
                    LVIS_SELECTED.0 | LVIS_FOCUSED.0,
                ),
                ..Default::default()
            };
            // SAFETY: Same-process synchronous ListView notification; item lives
            // through the call and actual Host WM_NOTIFY owns selected identity.
            unsafe {
                SendMessageW(
                    table.window(),
                    LVM_SETITEMSTATE,
                    Some(WPARAM(index)),
                    Some(LPARAM(&item as *const _ as isize)),
                );
            }
            require(
                table.selected() == Some(row.id),
                "Native selection did not retain exact accepted ID",
            )?;
            println!(
                "NETMAP_BROKER_CASE {{\"phase\":\"measured\",\"epoch\":\"{epoch:?}\",\"id\":{},\"pid\":{},\"rx\":{},\"tx\":{},\"rates\":true,\"native_cells\":9}}",
                row.id.0,
                std::process::id(),
                row.bytes.received,
                row.bytes.sent
            );
            first = Some((epoch, row.id));
            Ok(true)
        },
        "Measured broker loopback facts did not reach the ordinary native table",
    )?;
    wait(
        host,
        35,
        || Ok(pause.ready()),
        "Wrapper did not request subscription retirement",
    )?;
    let (epoch1, selected) = first.unwrap();
    host.app
        .native_broker_pause()
        .map_err(|e| failure(&e.to_string()))?;
    require(
        table(host)?.selected() == Some(selected),
        "Pausing subscription changed retained exact selection",
    )?;
    let traffic_result = traffic
        .try_recv()
        .map_err(|_| failure("Registered loopback worker did not finish during Work retirement"))?;
    traffic_result.map_err(|e| failure(&e))?;
    println!(
        "NETMAP_BROKER_CASE {{\"phase\":\"paused\",\"epoch\":\"{epoch1:?}\",\"selected\":{}}}",
        selected.0
    );
    wait(
        host,
        45,
        || Ok(resume.ready()),
        "Wrapper did not provide replacement collector",
    )?;
    host.app
        .native_broker_resume()
        .map_err(|e| failure(&e.to_string()))?;
    wait(
        host,
        25,
        || {
            let Some((Source::Broker(epoch), publication)) = host.app.native_broker_publication()
            else {
                return Ok(false);
            };
            if epoch == epoch1 {
                return Ok(false);
            }
            host.refresh_exploration();
            require(
                table(host)?.selected().is_none(),
                "A new authenticated source retained an old observation selection",
            )?;
            // A new collector can publish before lifecycle rundown completes.
            // Keep checking the cleared selection, but require complete coverage
            // within the existing bound rather than on its first publication.
            if publication.snapshot.coverage != Coverage::Complete {
                return Ok(false);
            }
            let records = SEQUENCES
                .lock()
                .map_err(|_| failure("Broker sequence diagnostics poisoned"))?;
            let records = records
                .as_ref()
                .ok_or_else(|| failure("Broker diagnostics not active"))?;
            require(
                !records.invalid && records.epochs.len() == 2,
                "Broker epochs/sequences did not advance monotonically",
            )?;
            for (epoch, first, last, count) in &records.epochs {
                println!(
                    "NETMAP_BROKER_SEQUENCE {{\"epoch\":\"{epoch:?}\",\"first\":{first},\"last\":{last},\"count\":{count}}}"
                );
            }
            println!(
                "NETMAP_BROKER_CASE {{\"phase\":\"reconnected\",\"epoch\":\"{epoch:?}\",\"selection_cleared\":true}}"
            );
            Ok(true)
        },
        "Same Application did not accept complete replacement broker coverage",
    )?;
    wait(
        host,
        25,
        || Ok(finish.ready()),
        "Wrapper did not finish subscriber lifetime check",
    )?;
    host.app
        .native_broker_pause()
        .map_err(|e| failure(&e.to_string()))?;
    println!("NETMAP_BROKER_CASE {{\"phase\":\"finished\",\"ordinary_window\":true}}");
    Ok(())
}

struct Traffic {
    local: crate::connections::Endpoint,
    remote: crate::connections::Endpoint,
    finished: mpsc::Receiver<std::result::Result<(), String>>,
}

fn traffic(host: &Host) -> Result<Traffic> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| failure(&e.to_string()))?;
    let mut client = TcpStream::connect_timeout(
        &listener.local_addr().map_err(|e| failure(&e.to_string()))?,
        Duration::from_secs(1),
    )
    .map_err(|e| failure(&e.to_string()))?;
    let (mut server, _) = listener.accept().map_err(|e| failure(&e.to_string()))?;
    for socket in [&client, &server] {
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(|e| failure(&e.to_string()))?;
        socket
            .set_write_timeout(Some(Duration::from_secs(1)))
            .map_err(|e| failure(&e.to_string()))?;
    }
    let local = crate::connections::Endpoint::new(
        client.local_addr().map_err(|e| failure(&e.to_string()))?,
    );
    let remote =
        crate::connections::Endpoint::new(client.peer_addr().map_err(|e| failure(&e.to_string()))?);
    let (sent, result) = mpsc::sync_channel(1);
    host.app
        .native_broker_traffic(move |cancelled| {
            let result = (|| -> std::io::Result<()> {
                let payload = [0x4e; 16384];
                let mut received = [0; 16384];
                while !cancelled.load(Ordering::Acquire) {
                    client.write_all(&payload)?;
                    server.read_exact(&mut received)?;
                    server.write_all(&payload[..8192])?;
                    client.read_exact(&mut received[..8192])?;
                    std::thread::sleep(Duration::from_millis(100));
                }
                Ok(())
            })()
            .map_err(|e| e.to_string());
            let _ = sent.send(result);
        })
        .map_err(|e| failure(&e.to_string()))?;
    Ok(Traffic {
        local,
        remote,
        finished: result,
    })
}

fn table(host: &Host) -> Result<&crate::app::exploration::Exploration> {
    host.exploration
        .get()
        .ok_or_else(|| failure("Native broker table missing"))
}

fn text(window: HWND, row: usize, column: usize) -> Result<String> {
    let mut buffer = [0u16; 512];
    let mut item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: row as i32,
        iSubItem: column as i32,
        pszText: PWSTR(buffer.as_mut_ptr()),
        cchTextMax: buffer.len() as i32,
        ..Default::default()
    };
    // SAFETY: Same-process live item/text storage through the actual native
    // owner-data GETDISPINFO callback, without cross-process pointer payloads.
    let found = unsafe {
        SendMessageW(
            window,
            LVM_GETITEMW,
            None,
            Some(LPARAM(&mut item as *mut _ as isize)),
        )
    }
    .0;
    require(found != 0, "Native broker row unavailable")?;
    let length = buffer
        .iter()
        .position(|&unit| unit == 0)
        .ok_or_else(|| failure("Native broker cell truncated"))?;
    Ok(String::from_utf16_lossy(&buffer[..length]))
}

fn wait(
    host: &Host,
    seconds: u64,
    mut ready: impl FnMut() -> Result<bool>,
    reason: &str,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if ready()? {
            return Ok(());
        }
        require(Instant::now() < deadline, reason)?;
        let mut message = MSG::default();
        // SAFETY: The owned 20ms timer bounds this normal UI-thread wait. Work
        // replies use the production Endpoint and ordinary dispatch order.
        unsafe {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                return Err(failure("Broker fixture Host retired unexpectedly"));
            }
            if !host.navigation_key(&message)
                && !IsDialogMessageW(host.window.get(), &message).as_bool()
            {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

fn require(value: bool, reason: &str) -> Result<()> {
    if value { Ok(()) } else { Err(failure(reason)) }
}
fn failure(reason: &str) -> Error {
    Error::new(E_FAIL, reason)
}

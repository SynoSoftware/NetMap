use super::*;

impl Dialog<'_> {
    pub(super) fn layout_pages(&self) {
        for page in Page::ALL {
            let window = self.page(page);
            if window.is_invalid() {
                continue;
            }
            // Showing/hiding the native scrollbar can change the wrapping width
            // once. Measure that resulting client area before placing children.
            for _ in 0..2 {
                let mut client = RECT::default();
                // SAFETY: Only this shell's retained child pages are laid out.
                if unsafe { GetClientRect(window, &mut client) }.is_err() {
                    break;
                }
                let (mut positions, height) = self.page_positions(page, client.right);
                control::scroll_range(window, height, client.bottom as u32);
                let mut after = RECT::default();
                let mut scroll = SCROLLINFO {
                    cbSize: size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_POS,
                    ..Default::default()
                };
                // SAFETY: Native scroll position remains the interaction owner.
                // Apply content coordinates relative to that accepted position.
                unsafe {
                    let _ = GetClientRect(window, &mut after);
                    let _ = GetScrollInfo(window, SB_VERT, &mut scroll);
                }
                for (_, rect) in &mut positions {
                    rect.top -= scroll.nPos;
                    rect.bottom -= scroll.nPos;
                }
                control::place(window, &positions);
                if after.right == client.right {
                    break;
                }
            }
        }
    }

    fn page_positions(&self, page: Page, width: i32) -> (Vec<(HWND, RECT)>, i32) {
        let window = self.page(page);
        let shell = self.window.get();
        let x = |du| native::units(shell, du, 0, 0, 0).left;
        let y = |du| native::units(shell, 0, du, 0, 0).top;
        let narrow = width < x(280);
        let inset = x(7);
        let content_width = (width - 2 * inset).max(1);
        let mut positions = Vec::new();
        let mut place = |id, left, top, width, height| {
            positions.push((
                control::item(window, id),
                RECT {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                },
            ));
        };
        place(IDC_HEADING, 0, 0, width, y(12));
        if matches!(page, Page::Desktop | Page::ScreenSaver) {
            place(IDC_SECTION, 0, y(22), width, y(12));
            let mut top = y(42);
            if page == Page::Desktop {
                let height = native::wrapped_height(
                    control::item(window, IDC_DESKTOP_ENABLED),
                    content_width,
                )
                .max(y(14));
                place(IDC_DESKTOP_ENABLED, inset, top, content_width, height);
                top += height + y(11);
                place(
                    IDC_MONITOR_LABEL,
                    inset,
                    top + if narrow { 0 } else { y(3) },
                    if narrow { content_width } else { x(92) },
                    y(10),
                );
                let left = if narrow { inset } else { x(106) };
                if narrow {
                    top += y(13);
                }
                let mut bounds = RECT::default();
                // SAFETY: Retain the native combo's selection-field height.
                unsafe {
                    let _ = GetWindowRect(control::item(window, IDC_MONITOR), &mut bounds);
                }
                place(
                    IDC_MONITOR,
                    left,
                    top,
                    (width - left - inset).max(1),
                    bounds.bottom - bounds.top,
                );
                top += y(32);
                let height = native::wrapped_height(control::item(window, IDC_NOTE), content_width)
                    .max(y(58));
                place(IDC_NOTE, inset, top, content_width, height);
                top += height + y(8);
            } else {
                let height =
                    native::wrapped_height(control::item(window, IDC_SAVER_STATUS), content_width)
                        .max(y(36));
                place(IDC_SAVER_STATUS, inset, top, content_width, height);
                top += height + y(8);
                for (id, preferred) in [
                    (IDC_INSTALL_SAVER, 126),
                    (IDC_SAVER_WINDOWS, 220),
                    (IDC_PREVIEW_SAVER, 90),
                ] {
                    let button_width = x(preferred).min(content_width);
                    let height = native::wrapped_height(
                        control::item(window, id),
                        (button_width - x(12)).max(1),
                    )
                    .max(y(10))
                        + y(6);
                    place(id, inset, top, button_width, height);
                    top += height + y(7);
                }
                top += y(7);
                let height =
                    native::wrapped_height(control::item(window, IDC_IMMEDIATE), content_width)
                        .max(y(52));
                place(IDC_IMMEDIATE, inset, top, content_width, height);
                top += height + y(8);
            }
            return (positions, top);
        }
        place(
            IDC_SECTION,
            0,
            y(if page == Page::Location { 20 } else { 22 }),
            width,
            y(12),
        );
        let (rows, column, field_width): (&[(u16, u16, Option<u16>)], _, _) = match page {
            Page::Map => (
                &[
                    (IDC_DETAIL_LABEL, IDC_DETAIL, None),
                    (IDC_SPEEDS_LABEL, IDC_SPEEDS, None),
                ],
                106,
                150,
            ),
            Page::Connections => (
                &[
                    (IDC_BRIGHT_LABEL, IDC_BRIGHT, Some(IDC_BRIGHT_SPIN)),
                    (IDC_FADE_LABEL, IDC_FADE, Some(IDC_FADE_SPIN)),
                ],
                124,
                78,
            ),
            Page::Location => (
                &[
                    (IDC_LATITUDE_LABEL, IDC_LATITUDE, None),
                    (IDC_LONGITUDE_LABEL, IDC_LONGITUDE, None),
                ],
                124,
                116,
            ),
            Page::Desktop | Page::ScreenSaver => unreachable!(),
        };
        let mut top = y(if page == Page::Location { 56 } else { 40 });
        if page == Page::Location {
            place(IDC_AUTOMATIC, inset, y(38), x(90), y(12));
            place(
                IDC_MANUAL,
                x(if narrow { 94 } else { 118 }),
                y(38),
                x(if narrow { 68 } else { 100 }),
                y(12),
            );
        }
        for &(label, editor, spin) in rows {
            let left = if narrow { inset } else { x(column) };
            let field_top = top + if narrow { y(13) } else { 0 };
            let field_width = x(field_width).min((width - left - inset).max(1));
            place(
                label,
                inset,
                top + if narrow { 0 } else { y(3) },
                if narrow {
                    content_width
                } else {
                    x(column - 14)
                },
                y(10),
            );
            let field_height = if page == Page::Map {
                let mut bounds = RECT::default();
                // SAFETY: The native combo owns its selection-field height;
                // CB_SETMINVISIBLE separately sizes its drop-down list.
                unsafe {
                    let _ = GetWindowRect(control::item(window, editor), &mut bounds);
                }
                bounds.bottom - bounds.top
            } else {
                y(14)
            };
            place(editor, left, field_top, field_width, field_height);
            if let Some(spin) = spin {
                place(spin, left + field_width - x(12), field_top, x(12), y(14));
            }
            top += y(if narrow {
                36
            } else if page == Page::Location {
                21
            } else {
                23
            });
        }
        let text_height = |id, minimum| {
            native::wrapped_height(control::item(window, id), content_width).max(y(minimum))
        };
        match page {
            Page::Map => {
                top += y(10);
                let height = text_height(IDC_NOTE, 42);
                place(IDC_NOTE, inset, top, content_width, height);
                top += height + y(8);
                place(IDC_LOCATION_SETTINGS, inset, top, x(90), y(16));
                top += y(24);
            }
            Page::Connections => {
                top += y(10);
                place(IDC_TRAFFIC, 0, top, width, y(12));
                top += y(20);
                let height = text_height(IDC_NOTE, 48);
                place(IDC_NOTE, inset, top, content_width, height);
                top += height + y(8);
            }
            Page::Location => {
                let height = text_height(IDC_ORIGIN_STATUS, 22);
                place(IDC_ORIGIN_STATUS, inset, top, content_width, height);
                top += height + y(4);
                let height = text_height(IDC_NOTE, 30);
                place(IDC_NOTE, inset, top, content_width, height);
                top += height + y(7);
                place(IDC_LOCATION_DATA, 0, top, width, y(12));
                top += y(19);
                let height = text_height(IDC_DATA_STATUS, 26);
                place(IDC_DATA_STATUS, inset, top, content_width, height);
                top += height + y(4);
                place(IDC_GET_CITY, inset, top, x(90), y(16));
                if width < x(208) {
                    top += y(23);
                    place(IDC_CANCEL_CITY_DATA, inset, top, x(90), y(16));
                } else {
                    place(IDC_CANCEL_CITY_DATA, x(104), top, x(90), y(16));
                }
                top += y(22);
                let height = text_height(IDC_IMMEDIATE, 28);
                place(IDC_IMMEDIATE, inset, top, content_width, height);
                top += height + y(4);
            }
            Page::Desktop | Page::ScreenSaver => unreachable!(),
        }
        (positions, top)
    }
}

//! Dev-only harness tool: injects synthetic touch, pen and mouse input into one process's window on Windows, and
//! reads that window's scale, pixels and frame, for scripts/record-traces.sh and scripts/touch-checks.sh
//! (docs/desktop-touch-windows.md). Touch and pen go through a synthetic pointer device, Microsoft's documented
//! `CreateSyntheticPointerDevice` / `InjectSyntheticPointerInput` (Windows 10 1809+), so the window gets real
//! `WM_POINTER*` messages and Windows' own promoted mouse copies; the mouse goes through `SendInput`.
//!
//! Usage: touch-inject launch <program> <log>
//!                                        start a program, its stderr to the log; prints its pid
//!        touch-inject park                move the mouse cursor to the screen's top-left corner, off the window
//!        touch-inject display <width> <height> <percent>
//!                                        set the display's resolution and scale; prints the old "width height percent"
//!        touch-inject <pid> <command>, every point in the process's window's logical px:
//!   raise                  keep the window above all others (the CI runner's own console can cover it)
//!   scale                  the window's scale factor (its DPI / 96)
//!   screenshot <file>      the client area as a raw RGBA image (scripts/locate-colour.py reads it)
//!   window-rect            the window's frame on screen, physical px: left top right bottom
//!   touch <frame>…         inject touch frames (frames.rs has the syntax); pen <frame>… a pen's
//!   click <x> <y>          a left mouse click there

#[cfg_attr(
    not(target_os = "windows"),
    allow(
        dead_code,
        reason = "only the Windows tool and the host tests use the engine"
    )
)]
mod frames;
#[cfg_attr(
    not(target_os = "windows"),
    allow(
        dead_code,
        reason = "only the Windows tool and the host tests use the steps"
    )
)]
mod scale;

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_tool::run(std::env::args().skip(1).collect())
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("touch-inject: Windows only");
    std::process::exit(2);
}

#[cfg(target_os = "windows")]
mod windows_tool {
    use std::{
        error::Error,
        thread::sleep,
        time::{Duration, Instant},
    };

    use windows::Win32::{
        Foundation::{HWND, LPARAM, POINT, RECT},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, ClientToScreen,
            CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject,
            GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
        },
        UI::{
            Controls::{
                CreateSyntheticPointerDevice, POINTER_FEEDBACK_DEFAULT, POINTER_TYPE_INFO,
                POINTER_TYPE_INFO_0,
            },
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
                SetProcessDpiAwarenessContext,
            },
            Input::{
                KeyboardAndMouse::{
                    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
                    MOUSEINPUT, SendInput,
                },
                Pointer::{
                    InjectSyntheticPointerInput, POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT,
                    POINTER_FLAG_INRANGE, POINTER_FLAG_UP, POINTER_FLAG_UPDATE, POINTER_FLAGS,
                    POINTER_INFO, POINTER_PEN_INFO, POINTER_TOUCH_INFO,
                },
            },
            WindowsAndMessaging::{
                EnumWindows, GetClientRect, GetWindowRect, GetWindowThreadProcessId, HWND_TOPMOST,
                IsWindowVisible, PEN_MASK_PRESSURE, POINTER_INPUT_TYPE, PT_PEN, PT_TOUCH,
                SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetCursorPos, SetWindowPos,
                TOUCH_MASK_CONTACTAREA, TOUCH_MASK_PRESSURE,
            },
        },
    };

    use windows::{
        Win32::{
            Devices::Display::{
                DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_DEVICE_INFO_TYPE,
                DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_PATH_SOURCE_INFO,
                DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo,
                GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
            },
            Graphics::Gdi::{
                CDS_TYPE, ChangeDisplaySettingsExW, DEVMODEW, DISP_CHANGE_SUCCESSFUL,
                DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW,
            },
        },
        core::PCWSTR,
    };

    use crate::frames::{self, Contact, Point, Pointers, parse_frame, to_screen};
    use crate::scale;

    // frames.rs writes WinUser.h's values out so its tests build on any host; they must be the windows crate's.
    const _: () = {
        assert!(frames::INRANGE == POINTER_FLAG_INRANGE.0);
        assert!(frames::INCONTACT == POINTER_FLAG_INCONTACT.0);
        assert!(frames::DOWN == POINTER_FLAG_DOWN.0);
        assert!(frames::UPDATE == POINTER_FLAG_UPDATE.0);
        assert!(frames::UP == POINTER_FLAG_UP.0);
    };

    /// A held contact is re-sent in place this often, like a 60 Hz digitizer; without updates Windows cancels it.
    const REPEAT: Duration = Duration::from_millis(16);

    /// Pressure of an injected contact (0 to 1024, POINTER_TOUCH_INFO and POINTER_PEN_INFO): half, so a pen's
    /// `force` reads 0.5.
    const PRESSURE: u32 = 512;

    /// How long a command waits for the process's window to open.
    const WINDOW_WAIT: Duration = Duration::from_secs(30);

    /// Most touch contacts injected at once (a pinch uses two).
    const MAX_TOUCHES: u32 = 10;

    pub fn run(args: Vec<String>) -> Result<(), Box<dyn Error>> {
        // Physical px everywhere, whatever the display scale.
        // SAFETY: called once, before any window or DPI query.
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }?;
        match args.as_slice() {
            [command] if command == "park" => {
                // SAFETY: plain call, no pointers.
                unsafe { SetCursorPos(0, 0) }?;
                return Ok(());
            }
            [command, program, log] if command == "launch" => {
                // Spawned here, not by the shell: Git Bash's `$!` names its own process, whose Windows pid can be
                // read before the program replaces it. Dropping the child leaves it running. It inherits this
                // tool's inheritable handles whatever its stdio, so a caller reads the pid from a file, not a pipe.
                let child = std::process::Command::new(program)
                    .stderr(std::fs::File::create(log)?)
                    .spawn()?;
                println!("{}", child.id());
                return Ok(());
            }
            [command, width, height, percent] if command == "display" => {
                return display(width.parse()?, height.parse()?, percent.parse()?);
            }
            _ => {}
        }
        let [pid, command, rest @ ..] = args.as_slice() else {
            return Err("usage: touch-inject <pid> <command> (see src/main.rs)".into());
        };
        let window = find_window(pid.parse()?)?;
        let Client {
            origin,
            width,
            height,
        } = client_area(window)?;
        // SAFETY: `window` is a live window handle found above.
        let scale = f64::from(unsafe { GetDpiForWindow(window) }) / 96.;
        match (command.as_str(), rest) {
            ("raise", []) => {
                // SAFETY: `window` is live; no pointers. Topmost needs no foreground rights, unlike activation.
                unsafe {
                    SetWindowPos(
                        window,
                        Some(HWND_TOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    )
                }?;
            }
            ("scale", []) => println!("{scale}"),
            ("screenshot", [file]) => std::fs::write(file, screenshot(origin, width, height)?)?,
            ("window-rect", []) => {
                let mut rect = RECT::default();
                // SAFETY: `window` is live and `rect` outlives the call.
                unsafe { GetWindowRect(window, &mut rect) }?;
                println!("{} {} {} {}", rect.left, rect.top, rect.right, rect.bottom);
            }
            ("touch", frames) => inject(PT_TOUCH, frames, origin, scale)?,
            ("pen", frames) => inject(PT_PEN, frames, origin, scale)?,
            ("click", [x, y]) => {
                let at = Point {
                    x: x.parse()?,
                    y: y.parse()?,
                };
                click(to_screen(at, origin, scale))?;
            }
            _ => return Err(format!("touch-inject: unknown command {command} {rest:?}").into()),
        }
        Ok(())
    }

    /// Sets the display's resolution, then its scale (the recommended scale can change with the resolution), and
    /// prints what they were. Windows offers 150 % only from about 1920 × 1080; the CI runner starts smaller.
    fn display(width: u32, height: u32, percent: u32) -> Result<(), Box<dyn Error>> {
        let mut was = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        // SAFETY: `was` is a DEVMODEW with its size set, alive for the call.
        unsafe { EnumDisplaySettingsW(PCWSTR::null(), ENUM_CURRENT_SETTINGS, &mut was) }.ok()?;
        let (current, min) = display_scale()?;
        let was_percent =
            scale::percent(current, min).ok_or("touch-inject: display scale off the steps")?;
        let mode = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            dmPelsWidth: width,
            dmPelsHeight: height,
            dmFields: DM_PELSWIDTH | DM_PELSHEIGHT,
            ..Default::default()
        };
        // SAFETY: `mode` is a DEVMODEW with its size set, alive for the call.
        let changed = unsafe {
            ChangeDisplaySettingsExW(PCWSTR::null(), Some(&mode), None, CDS_TYPE(0), None)
        };
        if changed != DISP_CHANGE_SUCCESSFUL {
            return Err(format!("touch-inject: no {width} x {height} mode ({changed:?})").into());
        }
        let (source, (_, min)) = (display_source()?, display_scale()?);
        let step = scale::relative_step(percent, min).ok_or("touch-inject: not a scale step")?;
        let set = ScalePacket {
            header: scale_header(source, SET_SCALE, size_of::<ScalePacket>()),
            relative: step,
        };
        // SAFETY: `set` is a packet of the size its header names, alive for the call.
        let result = unsafe { DisplayConfigSetDeviceInfo(&set.header) };
        if result != 0 {
            return Err(format!("touch-inject: {percent} % refused ({result})").into());
        }
        println!("{} {} {was_percent}", was.dmPelsWidth, was.dmPelsHeight);
        Ok(())
    }

    // Undocumented DisplayConfig packet types for the source's scale (see scale.rs).
    const GET_SCALE: i32 = -3;
    const SET_SCALE: i32 = -4;

    #[repr(C)]
    struct ScaleRangePacket {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        min: i32,
        current: i32,
        max: i32,
    }

    #[repr(C)]
    struct ScalePacket {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        relative: i32,
    }

    /// The first active display's source (the CI runner has one display).
    fn display_source() -> Result<DISPLAYCONFIG_PATH_SOURCE_INFO, Box<dyn Error>> {
        let (mut paths, mut modes) = (0, 0);
        // SAFETY: the counts outlive the call.
        unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths, &mut modes) }
            .ok()?;
        let mut path_infos = vec![DISPLAYCONFIG_PATH_INFO::default(); paths as usize];
        let mut mode_infos = vec![DISPLAYCONFIG_MODE_INFO::default(); modes as usize];
        // SAFETY: the arrays hold the counts passed with them.
        unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut paths,
                path_infos.as_mut_ptr(),
                &mut modes,
                mode_infos.as_mut_ptr(),
                None,
            )
        }
        .ok()?;
        Ok(path_infos
            .first()
            .ok_or("touch-inject: no active display")?
            .sourceInfo)
    }

    fn scale_header(
        source: DISPLAYCONFIG_PATH_SOURCE_INFO,
        kind: i32,
        size: usize,
    ) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
        DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(kind),
            size: size as u32,
            adapterId: source.adapterId,
            id: source.id,
        }
    }

    /// The display's scale step and its lowest step, both relative to the recommended one.
    fn display_scale() -> Result<(i32, i32), Box<dyn Error>> {
        let mut get = ScaleRangePacket {
            header: scale_header(display_source()?, GET_SCALE, size_of::<ScaleRangePacket>()),
            min: 0,
            current: 0,
            max: 0,
        };
        // SAFETY: `get` is a packet of the size its header names, alive for the call.
        let result = unsafe { DisplayConfigGetDeviceInfo(&mut get.header) };
        if result != 0 {
            return Err(format!("touch-inject: display scale unreadable ({result})").into());
        }
        Ok((get.current, get.min))
    }

    /// The process's visible top-level window, waiting up to `WINDOW_WAIT` for it to open.
    fn find_window(pid: u32) -> Result<HWND, Box<dyn Error>> {
        struct Search {
            pid: u32,
            found: Option<HWND>,
        }
        unsafe extern "system" fn visit(window: HWND, search: LPARAM) -> windows::core::BOOL {
            // SAFETY: `search` is the `&mut Search` EnumWindows was given, alive for the whole enumeration.
            let search = unsafe { &mut *(search.0 as *mut Search) };
            let mut pid = 0;
            // SAFETY: `window` comes from EnumWindows; `pid` outlives the call.
            unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
            // SAFETY: as above.
            if pid == search.pid && unsafe { IsWindowVisible(window) }.as_bool() {
                search.found = Some(window);
                return false.into();
            }
            true.into()
        }
        let started = Instant::now();
        loop {
            let mut search = Search { pid, found: None };
            // SAFETY: `visit` only reads `search` through the LPARAM during this call. EnumWindows reports the
            // early stop as an error, so its result says nothing; `found` does.
            let _ =
                unsafe { EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize)) };
            match search.found {
                Some(window) => return Ok(window),
                None if started.elapsed() < WINDOW_WAIT => sleep(Duration::from_millis(100)),
                None => {
                    return Err(format!("touch-inject: process {pid} has no visible window").into());
                }
            }
        }
    }

    /// The client area: its top-left on screen and its size, physical px.
    struct Client {
        origin: (i32, i32),
        width: i32,
        height: i32,
    }

    fn client_area(window: HWND) -> Result<Client, Box<dyn Error>> {
        let mut rect = RECT::default();
        let mut origin = POINT::default();
        // SAFETY: `window` is live; `rect` and `origin` outlive the calls.
        unsafe { GetClientRect(window, &mut rect) }?;
        // SAFETY: as above.
        unsafe { ClientToScreen(window, &mut origin) }.ok()?;
        Ok(Client {
            origin: (origin.x, origin.y),
            width: rect.right,
            height: rect.bottom,
        })
    }

    /// The screen under the client area: a 16-byte header (width, height, two zero words, little endian) and RGBA
    /// rows, the raw layout locate-colour.py reads (Android's screencap has the same).
    fn screenshot(origin: (i32, i32), width: i32, height: i32) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        // SAFETY: every handle is created here and released before returning; `bgra` holds width × height
        // 32-bit pixels, what GetDIBits writes for this top-down BITMAPINFO.
        let rows = unsafe {
            let screen = GetDC(None);
            let memory = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, width, height);
            let old = SelectObject(memory, bitmap.into());
            // CAPTUREBLT: include layered and composed windows, which GPUI's DirectComposition window is.
            let copied = BitBlt(
                memory,
                0,
                0,
                width,
                height,
                Some(screen),
                origin.0,
                origin.1,
                SRCCOPY | CAPTUREBLT,
            );
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            SelectObject(memory, old);
            let rows = GetDIBits(
                memory,
                bitmap,
                0,
                height as u32,
                Some(bgra.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            );
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(memory);
            ReleaseDC(None, screen);
            copied?;
            rows
        };
        if rows != height {
            return Err("touch-inject: GetDIBits copied no screen".into());
        }
        let mut raw = Vec::with_capacity(16 + bgra.len());
        for word in [width as u32, height as u32, 0, 0] {
            raw.extend_from_slice(&word.to_le_bytes());
        }
        for pixel in bgra.as_chunks::<4>().0 {
            raw.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
        Ok(raw)
    }

    /// Injects each frame, then holds it its `ms`, re-sending held pointers in place every `REPEAT`.
    fn inject(
        kind: POINTER_INPUT_TYPE,
        texts: &[String],
        origin: (i32, i32),
        scale: f64,
    ) -> Result<(), Box<dyn Error>> {
        let frames = texts
            .iter()
            .map(|text| parse_frame(text))
            .collect::<Result<Vec<_>, _>>()?;
        let max = if kind == PT_PEN { 1 } else { MAX_TOUCHES };
        // SAFETY: plain call; the handle is freed when `device` drops (windows' Free impl).
        let device = unsafe { CreateSyntheticPointerDevice(kind, max, POINTER_FEEDBACK_DEFAULT) }?;
        let mut pointers = Pointers::default();
        let send = |contacts: Vec<Contact>| -> Result<(), Box<dyn Error>> {
            if contacts.is_empty() {
                return Ok(());
            }
            let infos: Vec<POINTER_TYPE_INFO> = contacts
                .iter()
                .map(|contact| pointer_type_info(kind, contact, origin, scale))
                .collect();
            // SAFETY: `device` is live and `infos` is a valid slice of `kind`'s pointers.
            unsafe { InjectSyntheticPointerInput(device, &infos) }?;
            Ok(())
        };
        let mut due = Instant::now();
        let held = frames::parse_frame("0")?;
        for frame in &frames {
            send(pointers.step(frame)?)?;
            let end = due + Duration::from_millis(frame.ms);
            loop {
                due = (due + REPEAT).min(end);
                sleep(due.saturating_duration_since(Instant::now()));
                if due == end {
                    break;
                }
                if pointers.any_in_range() {
                    send(pointers.step(&held)?)?;
                }
            }
        }
        Ok(())
    }

    fn pointer_type_info(
        kind: POINTER_INPUT_TYPE,
        contact: &Contact,
        origin: (i32, i32),
        scale: f64,
    ) -> POINTER_TYPE_INFO {
        let (x, y) = to_screen(contact.at, origin, scale);
        let pointer_info = POINTER_INFO {
            pointerType: kind,
            pointerId: contact.slot,
            pointerFlags: POINTER_FLAGS(contact.flags),
            ptPixelLocation: POINT { x, y },
            ..Default::default()
        };
        let pressure = if contact.touching { PRESSURE } else { 0 };
        let anonymous = if kind == PT_PEN {
            POINTER_TYPE_INFO_0 {
                penInfo: POINTER_PEN_INFO {
                    pointerInfo: pointer_info,
                    penMask: PEN_MASK_PRESSURE,
                    pressure,
                    ..Default::default()
                },
            }
        } else {
            POINTER_TYPE_INFO_0 {
                touchInfo: POINTER_TOUCH_INFO {
                    pointerInfo: pointer_info,
                    touchMask: TOUCH_MASK_CONTACTAREA | TOUCH_MASK_PRESSURE,
                    // A 4 px finger around the point.
                    rcContact: RECT {
                        left: x - 2,
                        top: y - 2,
                        right: x + 2,
                        bottom: y + 2,
                    },
                    pressure,
                    ..Default::default()
                },
            }
        };
        POINTER_TYPE_INFO {
            r#type: kind,
            Anonymous: anonymous,
        }
    }

    /// A real (not promoted) left click: the cursor moves there, then a press and a release.
    fn click((x, y): (i32, i32)) -> Result<(), Box<dyn Error>> {
        // SAFETY: plain call, no pointers.
        unsafe { SetCursorPos(x, y) }?;
        let button = |flags| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        let inputs = [button(MOUSEEVENTF_LEFTDOWN), button(MOUSEEVENTF_LEFTUP)];
        // SAFETY: `inputs` is a valid slice of INPUTs of the size passed.
        let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err("touch-inject: SendInput was blocked".into());
        }
        Ok(())
    }
}

//! Fotofono for Windows: a WebView2 window around docs/index.html (the same page as the website),
//! plus what a browser can't do well: native "Save as", "Open with", offline fonts, remembered window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod strings;

use std::{
    cell::RefCell,
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    platform::windows::{IconExtWindows, WindowBuilderExtWindows, WindowExtWindows},
    window::{Icon, Theme, Window, WindowBuilder},
};
use webview2_com::{
    take_pwstr, DownloadStartingEventHandler, Microsoft::Web::WebView2::Win32::*,
    ProcessFailedEventHandler, StateChangedEventHandler, WebMessageReceivedEventHandler,
};
use windows::{
    core::{Interface, HSTRING, PCWSTR, PWSTR},
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::{
            Dwm::{DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE},
            Gdi::{MonitorFromRect, MONITOR_DEFAULTTONULL},
        },
        System::SystemInformation::GetLocalTime,
        UI::{
            Shell::{FOLDERID_Music, SHGetKnownFolderPath, ShellExecuteW, KF_FLAG_DEFAULT},
            WindowsAndMessaging::{
                GetWindowPlacement, SetWindowPlacement, SystemParametersInfoW, SPI_GETWORKAREA,
                SW_SHOWMAXIMIZED, SW_SHOWNORMAL, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WINDOWPLACEMENT,
            },
        },
    },
};
use wry::{
    NewWindowResponse, PageLoadEvent, WebContext, WebViewBuilder, WebViewBuilderExtWindows,
    WebViewExtWindows,
};

const HOST: &str = "fotofono.local";
const ORIGIN: &str = "https://fotofono.local/";
/// Smallest window, in pixels (the same as the .NET version's MinimumSize).
const MIN_W: i32 = 480;
const MIN_H: i32 = 560;
/// Chassis colour of the interface: window, title bar and page background
const CHASSIS: (u8, u8, u8) = (0x12, 0x14, 0x18);

// ---- the page and its files, embedded in the executable (works offline) ----

const INDEX_HTML: &str = include_str!("../../docs/index.html");
const FONTS_CSS: &str = include_str!("../fonts/fonts.css");
const ICON_ICO: &[u8] = include_bytes!("../fotofono.ico");
const FONTS: &[(&str, &[u8])] = &[
    ("ChakraPetch-500.woff2", include_bytes!("../fonts/ChakraPetch-500.woff2")),
    ("ChakraPetch-600.woff2", include_bytes!("../fonts/ChakraPetch-600.woff2")),
    ("ChakraPetch-700.woff2", include_bytes!("../fonts/ChakraPetch-700.woff2")),
    ("IBMPlexSans-400.woff2", include_bytes!("../fonts/IBMPlexSans-400.woff2")),
    ("IBMPlexSans-500.woff2", include_bytes!("../fonts/IBMPlexSans-500.woff2")),
    ("ShareTechMono-400.woff2", include_bytes!("../fonts/ShareTechMono-400.woff2")),
];

/// FOTOFONO_DATA lets you use another data folder (for example for automated tests)
pub fn data_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| match std::env::var_os("FOTOFONO_DATA") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("Fotofono"),
    })
}

enum UserEvent {
    Title(String),
    PageLoaded(String),
    Message(String),
    Download,
    Reload,
}

thread_local! {
    /// Downloads waiting for the "Save as" dialog: handled from the event loop, not inside the WebView2 callback
    static PENDING: RefCell<VecDeque<(ICoreWebView2DownloadStartingEventArgs, ICoreWebView2Deferral)>> =
        RefCell::new(VecDeque::new());
}

fn main() {
    // An unexpected error never closes the app silently: it is logged and shown
    std::panic::set_hook(Box::new(|info| {
        log_error(&format!("panic: {info}"));
        let log = data_dir().join("errors.log");
        message(
            rfd::MessageLevel::Error,
            &format!("{}\n\n{info}\n\n{} {}", strings::get("unexpected"), strings::get("details"), log.display()),
        );
    }));

    if let Err(e) = run() {
        log_error(&format!("startup: {e}"));
        message(rfd::MessageLevel::Error, &format!("{}\n\n{e}", strings::get("engineFail")));
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // "Open with Fotofono", or an image dropped on the executable's icon
    let mut start_file = std::env::args_os().skip(1).map(PathBuf::from).find(|p| p.is_file());

    if !webview2_installed() {
        let yes = rfd::MessageDialog::new()
            .set_title("Fotofono")
            .set_description(strings::get("noWebView"))
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if yes == rfd::MessageDialogResult::Yes {
            open_external("https://go.microsoft.com/fwlink/p/?LinkId=2124703");
        }
        return Ok(());
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let (r, g, b) = CHASSIS;
    let window = WindowBuilder::new()
        .with_title("Fotofono")
        .with_window_icon(Icon::from_resource(1, None).ok())
        .with_min_inner_size(LogicalSize::new(MIN_W as f64 - 16.0, MIN_H as f64 - 40.0))
        .with_theme(Some(Theme::Dark))
        .with_background_color((r, g, b, 255))
        // files dropped on the window go to the page (HTML drag and drop), not to the window
        .with_drag_and_drop(false)
        .with_visible(false)
        .build(&event_loop)?;
    let hwnd = HWND(window.hwnd() as *mut _);
    style_title_bar(hwnd);
    restore_placement(hwnd);
    window.set_visible(true);

    let site = extract_site()?;
    let mut context = WebContext::new(Some(data_dir().join("WebView2")));
    let (p_title, p_load) = (proxy.clone(), proxy.clone());
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_background_color((r, g, b, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_default_context_menus(false)
        .with_browser_accelerator_keys(false)
        .with_general_autofill_enabled(false)
        // Shown while the page loads
        .with_html(splash_html())
        .with_navigation_handler(|uri| {
            if is_own(&uri) || uri.starts_with("about:") || uri.starts_with("data:") {
                return true;
            }
            open_external(&uri);
            false
        })
        .with_new_window_req_handler(|uri, _| {
            open_external(&uri);
            NewWindowResponse::Deny
        })
        .with_document_title_changed_handler(move |t| {
            let _ = p_title.send_event(UserEvent::Title(t));
        })
        .with_on_page_load_handler(move |ev, url| {
            if matches!(ev, PageLoadEvent::Finished) {
                let _ = p_load.send_event(UserEvent::PageLoaded(url));
            }
        })
        .build(&window)?;

    let core = webview.webview();
    unsafe { setup_core(&core, &site, &proxy)? };
    webview.load_url(&format!("{ORIGIN}index.html"))?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let _keep_alive = (&webview, &context);
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                save_placement(hwnd);
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Title(t)) => {
                let t = t.trim();
                window.set_title(if t.is_empty() || t.contains(':') { "Fotofono" } else { t });
            }
            Event::UserEvent(UserEvent::PageLoaded(url)) => {
                if is_own(&url) {
                    if let Some(f) = start_file.take() {
                        open_image(&core, &f);
                    }
                }
            }
            Event::UserEvent(UserEvent::Message(json)) => on_web_message(&json),
            Event::UserEvent(UserEvent::Download) => handle_downloads(&window, &core),
            Event::UserEvent(UserEvent::Reload) => unsafe {
                let _ = core.Reload();
            },
            _ => {}
        }
    });
}

fn is_own(uri: &str) -> bool {
    uri.len() >= ORIGIN.len() && uri[..ORIGIN.len()].eq_ignore_ascii_case(ORIGIN)
}

/// WebView2 settings and events that wry does not expose.
unsafe fn setup_core(core: &ICoreWebView2, site: &Path, proxy: &EventLoopProxy<UserEvent>) -> windows::core::Result<()> {
    // The page files are extracted once to a folder and served from a virtual host:
    // the fastest option, and the same origin as the .NET version (saved settings carry over)
    let core3: ICoreWebView2_3 = core.cast()?;
    core3.SetVirtualHostNameToFolderMapping(
        &HSTRING::from(HOST),
        &HSTRING::from(site.as_os_str()),
        COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY,
    )?;

    let settings = core.Settings()?;
    if let Ok(s4) = settings.cast::<ICoreWebView2Settings4>() {
        let _ = s4.SetIsPasswordAutosaveEnabled(false);
    }
    // The page is embedded and external links open in the default browser:
    // checking every navigation with SmartScreen protects nothing here and slows the startup
    if let Ok(s8) = settings.cast::<ICoreWebView2Settings8>() {
        let _ = s8.SetIsReputationCheckingRequired(false);
    }

    let mut token = 0i64;

    // Messages from the page: JSON objects, which wry's string-only IPC would drop
    let p = proxy.clone();
    core.add_WebMessageReceived(
        &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                let mut json = PWSTR::null();
                args.WebMessageAsJson(&mut json)?;
                let _ = p.send_event(UserEvent::Message(take_pwstr(json)));
            }
            Ok(())
        })),
        &mut token,
    )?;

    // Saving sounds with the native "Save as" dialog
    let p = proxy.clone();
    let core4: ICoreWebView2_4 = core.cast()?;
    core4.add_DownloadStarting(
        &DownloadStartingEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let deferral = args.GetDeferral()?;
            PENDING.with(|q| q.borrow_mut().push_back((args, deferral)));
            let _ = p.send_event(UserEvent::Download);
            Ok(())
        })),
        &mut token,
    )?;

    let p = proxy.clone();
    core.add_ProcessFailed(
        &ProcessFailedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
            args.ProcessFailedKind(&mut kind)?;
            let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON::default();
            if let Ok(a2) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() {
                let _ = a2.Reason(&mut reason);
            }
            log_error(&format!("WebView2 ProcessFailed: kind {} / reason {}", kind.0, reason.0));
            if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
                || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE
            {
                let _ = p.send_event(UserEvent::Reload);
            }
            Ok(())
        })),
        &mut token,
    )?;
    Ok(())
}

// ---- messages from the page ----

fn on_web_message(json: &str) {
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(v) => {
            if v.get("type").and_then(|t| t.as_str()) == Some("lang") {
                if let Some(l) = v.get("lang").and_then(|l| l.as_str()) {
                    strings::set(l);
                }
            }
        }
        Err(e) => log_error(&format!("web message: {e}")),
    }
}

/// Shows a message in the page; the page translates the key into the current language.
fn toast(core: &ICoreWebView2, key: &str, arg: Option<&str>) {
    post(core, &serde_json::json!({ "type": "toast", "key": key, "arg": arg }));
}

fn post(core: &ICoreWebView2, msg: &serde_json::Value) {
    let _ = unsafe { core.PostWebMessageAsJson(&HSTRING::from(msg.to_string())) };
}

fn open_image(core: &ICoreWebView2, file: &Path) {
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match std::fs::metadata(file) {
        Ok(m) if m.len() > 80 * 1024 * 1024 => return toast(core, "msg.tooBig", Some(&name)),
        _ => {}
    }
    match std::fs::read(file) {
        Ok(bytes) => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            post(core, &serde_json::json!({ "type": "open", "name": name, "mime": mime_for(file), "b64": b64 }));
        }
        Err(e) => {
            log_error(&format!("open {}: {e}", file.display()));
            toast(core, "msg.cantOpen", Some(&name));
        }
    }
}

fn handle_downloads(window: &Window, core: &ICoreWebView2) {
    while let Some((args, deferral)) = PENDING.with(|q| q.borrow_mut().pop_front()) {
        if let Err(e) = unsafe { save_download(window, core, &args) } {
            log_error(&format!("save: {e}"));
            let _ = unsafe { args.SetCancel(true) };
            toast(core, "msg.saveFailed", None);
        }
        let _ = unsafe { deferral.Complete() };
    }
}

unsafe fn save_download(
    window: &Window,
    core: &ICoreWebView2,
    args: &ICoreWebView2DownloadStartingEventArgs,
) -> windows::core::Result<()> {
    let mut p = PWSTR::null();
    args.ResultFilePath(&mut p)?;
    let suggested_path = PathBuf::from(take_pwstr(p));
    let suggested = suggested_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = suggested_path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let (label, ext) = match ext.as_str() {
        "zip" => (strings::get("zip"), "zip"),
        "mid" => (strings::get("midi"), "mid"),
        _ => (strings::get("wav"), "wav"),
    };
    let mut dlg = rfd::FileDialog::new()
        .set_title(strings::get("saveTitle"))
        .set_file_name(&suggested)
        .add_filter(format!("{label} (*.{ext})"), &[ext])
        .set_parent(window);
    if let Some(music) = known_folder(&FOLDERID_Music) {
        dlg = dlg.set_directory(music);
    }
    let Some(target) = dlg.save_file() else {
        return args.SetCancel(true);
    };

    args.SetResultFilePath(&HSTRING::from(target.as_os_str()))?;
    args.SetHandled(true)?;
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let core = core.clone();
    let mut token = 0i64;
    args.DownloadOperation()?.add_StateChanged(
        &StateChangedEventHandler::create(Box::new(move |op, _| {
            let Some(op) = op else { return Ok(()) };
            let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
            op.State(&mut state)?;
            if state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
                toast(&core, "msg.saved", Some(&name));
            } else if state == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
                toast(&core, "msg.saveFailed", None);
            }
            Ok(())
        })),
        &mut token,
    )
}

fn extract_site() -> std::io::Result<PathBuf> {
    let page = build_page();
    let hash = Sha256::digest(page.as_bytes());
    let tag: String = hash.iter().take(6).map(|b| format!("{b:02X}")).collect();
    let root = data_dir().join("app");
    let dir = root.join(&tag);
    if !dir.join("index.html").exists() {
        let tmp = root.join(format!("{tag}.tmp"));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("fonts"))?;
        for (name, bytes) in FONTS {
            std::fs::write(tmp.join("fonts").join(name), bytes)?;
        }
        std::fs::write(tmp.join("favicon.ico"), ICON_ICO)?;
        std::fs::write(tmp.join("index.html"), page.as_bytes())?;
        if let Err(e) = std::fs::rename(&tmp, &dir) {
            // another instance just created it
            if !dir.join("index.html").exists() {
                return Err(e);
            }
            let _ = std::fs::remove_dir_all(&tmp);
        }
    }
    // previous versions of the page are no longer needed
    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if !n.eq_ignore_ascii_case(&tag) && !n.ends_with(".tmp") {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(dir)
}

/// Same page as the website; here the fonts come from the executable instead of Google Fonts.
fn build_page() -> String {
    let re = regex::Regex::new(r"<link[^>]+fonts\.(googleapis|gstatic)\.com[^>]*>\s*").unwrap();
    let mut page = re.replace_all(INDEX_HTML, "").into_owned();
    if let Some(at) = page.to_ascii_lowercase().find("</head>") {
        page.insert_str(at, &format!("<style>{FONTS_CSS}</style>\n"));
    }
    page
}

fn splash_html() -> String {
    format!(
        "<!doctype html><meta charset=utf-8><title>Fotofono</title>\
         <body style=\"margin:0;height:100vh;display:grid;place-items:center;background:#121418;\
         color:#ffb23e;font:17px Bahnschrift,'Segoe UI',sans-serif;cursor:default;user-select:none\">{}</body>",
        strings::get("splash")
    )
}

// ---- window position and size, remembered between launches ----

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Placement {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    max: bool,
}

fn placement_file() -> PathBuf {
    data_dir().join("window.json")
}

fn restore_placement(hwnd: HWND) {
    let saved = std::fs::read_to_string(placement_file())
        .ok()
        .and_then(|s| serde_json::from_str::<Placement>(&s).ok())
        .filter(|p| p.w >= MIN_W && p.h >= MIN_H)
        .filter(|p| {
            // only if the title bar lands on a screen that is connected now
            let bar = RECT { left: p.x + 40, top: p.y, right: p.x + 40 + (p.w - 80).max(1), bottom: p.y + 30 };
            !unsafe { MonitorFromRect(&bar, MONITOR_DEFAULTTONULL) }.is_invalid()
        });

    let (rect, max) = match saved {
        Some(p) => (RECT { left: p.x, top: p.y, right: p.x + p.w, bottom: p.y + p.h }, p.max),
        None => {
            let mut area = RECT { left: 0, top: 0, right: 1600, bottom: 1000 };
            let _ = unsafe {
                SystemParametersInfoW(SPI_GETWORKAREA, 0, Some(&mut area as *mut _ as _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))
            };
            let (aw, ah) = (area.right - area.left, area.bottom - area.top);
            let (w, h) = (1400.min(aw - 80), 940.min(ah - 60));
            let (x, y) = (area.left + (aw - w) / 2, area.top + (ah - h) / 2);
            (RECT { left: x, top: y, right: x + w, bottom: y + h }, false)
        }
    };
    let wp = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        showCmd: (if max { SW_SHOWMAXIMIZED } else { SW_SHOWNORMAL }).0 as u32,
        rcNormalPosition: rect,
        ..Default::default()
    };
    let _ = unsafe { SetWindowPlacement(hwnd, &wp) };
}

fn save_placement(hwnd: HWND) {
    let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
    if unsafe { GetWindowPlacement(hwnd, &mut wp) }.is_err() {
        return;
    }
    let r = wp.rcNormalPosition;
    let p = Placement {
        x: r.left,
        y: r.top,
        w: r.right - r.left,
        h: r.bottom - r.top,
        max: wp.showCmd == SW_SHOWMAXIMIZED.0 as u32,
    };
    let result = std::fs::create_dir_all(data_dir())
        .and_then(|_| std::fs::write(placement_file(), serde_json::to_string(&p).unwrap_or_default()));
    if let Err(e) = result {
        log_error(&format!("window.json: {e}"));
    }
}

// ---- helpers ----

/// The interface is a dark rack: window and title bar share the chassis colour.
fn style_title_bar(hwnd: HWND) {
    let (r, g, b) = CHASSIS;
    let dark: i32 = 1;
    let caption: u32 = r as u32 | (g as u32) << 8 | (b as u32) << 16; // COLORREF
    let text: u32 = 0x00ECE7E4; // #e4e7ec
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const _ as _, 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, &caption as *const _ as _, 4); // Windows 11
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, &text as *const _ as _, 4); // Windows 11
    }
}

fn webview2_installed() -> bool {
    let mut v = PWSTR::null();
    let ok = unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut v) }.is_ok();
    ok && !v.is_null() && !take_pwstr(v).is_empty()
}

fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.ok().map(|p| PathBuf::from(take_pwstr(p)))
}

fn open_external(uri: &str) {
    let lower = uri.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        unsafe {
            ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(uri), None, None, SW_SHOWNORMAL);
        }
    }
}

fn mime_for(file: &Path) -> &'static str {
    match file.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg" | "jfif") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("avif") => "image/avif",
        Some("tif" | "tiff") => "image/tiff",
        _ => "application/octet-stream",
    }
}

fn message(level: rfd::MessageLevel, text: &str) {
    rfd::MessageDialog::new().set_title("Fotofono").set_description(text).set_level(level).show();
}

pub fn log_error(msg: &str) {
    let t = unsafe { GetLocalTime() };
    let line = format!(
        "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}] {msg}\n\n",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    );
    let _ = std::fs::create_dir_all(data_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(data_dir().join("errors.log")) {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

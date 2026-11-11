//! Placement sous les icônes du bureau et détection de ce qui est visible.
//!
//! Deux organisations du bureau existent :
//! - avant Windows 11 24H2 : le message 0x052C fait créer à l'Explorateur une
//!   fenêtre `WorkerW` de premier niveau, derrière celle des icônes ; on s'y
//!   accroche comme enfant ;
//! - depuis 24H2 : les icônes (`SHELLDLL_DefView`) et le `WorkerW` sont des
//!   enfants de `Progman` ; on s'insère entre les deux.

use crate::render::SurfaceDesc;
use crate::util::{class_name, WStr};
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const HOST_CLASS: &str = "OndeFond";

#[derive(Clone, Copy, Debug)]
pub struct Host {
    pub parent: HWND,
    /// Disposition 24H2 : fenêtre des icônes, au-dessus de laquelle on ne doit pas passer.
    pub defview: Option<HWND>,
    pub workerw: Option<HWND>,
}

fn find(parent: Option<HWND>, after: Option<HWND>, class: PCWSTR) -> Option<HWND> {
    unsafe { FindWindowExW(parent, after, class, PCWSTR::null()) }.ok().filter(|h| !h.is_invalid())
}

pub fn find_host() -> Option<Host> {
    let progman = find(None, None, w!("Progman"))?;
    unsafe {
        let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0xD), LPARAM(1), SMTO_NORMAL, 1000, None);
        let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0), LPARAM(0), SMTO_NORMAL, 1000, None);
    }
    // Disposition 24H2 et suivantes.
    if let Some(defview) = find(Some(progman), None, w!("SHELLDLL_DefView")) {
        let workerw = find(Some(progman), None, w!("WorkerW"));
        return Some(Host { parent: progman, defview: Some(defview), workerw });
    }
    // Disposition historique : le WorkerW qui suit la fenêtre contenant les icônes.
    let mut found: Option<HWND> = None;
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if find(Some(hwnd), None, w!("SHELLDLL_DefView")).is_some() {
            let out = &mut *(lparam.0 as *mut Option<HWND>);
            *out = find(None, Some(hwnd), w!("WorkerW"));
            return false.into();
        }
        true.into()
    }
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut found as *mut _ as isize));
    }
    match found {
        Some(w) => Some(Host { parent: w, defview: None, workerw: Some(w) }),
        None => Some(Host { parent: progman, defview: None, workerw: None }),
    }
}

#[derive(Clone, Debug)]
pub struct Monitor {
    pub handle: HMONITOR,
    pub rect: RECT,
    pub work: RECT,
    pub refresh: u32,
}

pub fn monitors() -> Vec<Monitor> {
    let mut out: Vec<Monitor> = Vec::new();
    unsafe extern "system" fn each(hmon: HMONITOR, _: HDC, _: *mut RECT, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam.0 as *mut Vec<Monitor>);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut mi as *mut _ as *mut MONITORINFO).as_bool() {
            let mut dm = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
            let refresh =
                if EnumDisplaySettingsW(PCWSTR(mi.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm).as_bool() {
                    dm.dmDisplayFrequency
                } else {
                    60
                };
            out.push(Monitor { handle: hmon, rect: mi.monitorInfo.rcMonitor, work: mi.monitorInfo.rcWork, refresh });
        }
        true.into()
    }
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut out as *mut _ as isize));
    }
    // Ordre stable : de gauche à droite.
    out.sort_by_key(|m| (m.rect.left, m.rect.top));
    out
}

unsafe extern "system" fn host_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let _ = ValidateRect(Some(hwnd), None);
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn register_class() {
    unsafe {
        let class = WStr::new(HOST_CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(host_proc),
            hInstance: GetModuleHandleW(None).unwrap_or_default().into(),
            lpszClassName: class.pcwstr(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
}

/// Crée une fenêtre hôte par écran sous les icônes.
pub fn create_hosts(host: &Host, mons: &[Monitor]) -> Vec<SurfaceDesc> {
    let class = WStr::new(HOST_CLASS);
    let mut out = Vec::new();
    for m in mons {
        // Coordonnées écran → coordonnées client du parent.
        let mut pts = [POINT { x: m.rect.left, y: m.rect.top }, POINT { x: m.rect.right, y: m.rect.bottom }];
        unsafe { MapWindowPoints(None, Some(host.parent), &mut pts) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_NOPARENTNOTIFY,
                class.pcwstr(),
                w!("Onde"),
                WS_CHILD | WS_VISIBLE | WS_DISABLED | WS_CLIPSIBLINGS,
                pts[0].x,
                pts[0].y,
                pts[1].x - pts[0].x,
                pts[1].y - pts[0].y,
                Some(host.parent),
                None,
                GetModuleHandleW(None).ok().map(|h| h.into()),
                None,
            )
        };
        let Ok(hwnd) = hwnd else {
            crate::log!("création de la fenêtre hôte impossible : {:?}", hwnd.err());
            continue;
        };
        unsafe {
            if let Some(defview) = host.defview {
                // Juste sous les icônes, au-dessus de l'image de fond de Windows.
                let _ = SetWindowPos(hwnd, Some(defview), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            } else {
                let _ = SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }
        out.push(SurfaceDesc { hwnd: hwnd.0 as isize, rect: m.rect, refresh: m.refresh });
    }
    if let (Some(_), Some(ww)) = (host.defview, host.workerw) {
        // L'image de fond de Windows passe sous toutes nos fenêtres.
        unsafe {
            let _ = SetWindowPos(ww, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
    out
}

pub fn destroy_hosts(hosts: &[isize]) {
    for &h in hosts {
        unsafe {
            let _ = DestroyWindow(HWND(h as *mut _));
        }
    }
}

/// Fait repeindre le fond de Windows après notre départ.
pub fn repaint(host: &Host) {
    unsafe {
        let flags = RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW;
        let _ = RedrawWindow(Some(host.parent), None, None, flags);
        if let Some(w) = host.workerw {
            let _ = RedrawWindow(Some(w), None, None, flags);
        }
    }
}

const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
    "TopLevelWindowForOverflowXamlIsland",
];

fn covers(outer: &RECT, inner: &RECT) -> bool {
    const TOL: i32 = 4;
    outer.left <= inner.left + TOL
        && outer.top <= inner.top + TOL
        && outer.right >= inner.right - TOL
        && outer.bottom >= inner.bottom - TOL
}

struct Scan<'a> {
    mons: &'a [Monitor],
    covered: Vec<bool>,
}

/// Pour chaque écran : est-il entièrement caché par une fenêtre
/// (agrandie ou plein écran) ? Parcours des fenêtres de premier niveau,
/// quelques microsecondes.
pub fn covered_monitors(mons: &[Monitor]) -> Vec<bool> {
    let mut scan = Scan { mons, covered: vec![false; mons.len()] };
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let scan = &mut *(lparam.0 as *mut Scan);
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return true.into();
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        // Calques transparents aux clics (surimpressions de jeux, d'enregistreurs…).
        if ex & WS_EX_TRANSPARENT.0 != 0 || ex & WS_EX_TOOLWINDOW.0 != 0 && ex & WS_EX_APPWINDOW.0 == 0 {
            return true.into();
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, 4);
        if cloaked != 0 {
            return true.into();
        }
        if SHELL_CLASSES.contains(&class_name(hwnd).as_str()) {
            return true.into();
        }
        let mut r = RECT::default();
        if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut _ as *mut _, 16).is_err()
            && GetWindowRect(hwnd, &mut r).is_err()
        {
            return true.into();
        }
        let zoomed = IsZoomed(hwnd).as_bool();
        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL);
        for (i, m) in scan.mons.iter().enumerate() {
            if covers(&r, &m.work) || (zoomed && hmon == m.handle) {
                scan.covered[i] = true;
            }
        }
        true.into()
    }
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut scan as *mut _ as isize));
    }
    scan.covered
}

/// Le bureau est-il au premier plan (aucune application active) ?
pub fn desktop_in_front() -> bool {
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
        return true;
    }
    matches!(class_name(fg).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
}

/// Diagnostic : arborescence des fenêtres du bureau.
pub fn describe() -> String {
    let mut s = String::new();
    let Some(progman) = find(None, None, w!("Progman")) else { return "Progman introuvable".into() };
    s.push_str(&format!("Progman {:?} ex={:#x}\n", progman.0, unsafe { GetWindowLongW(progman, GWL_EXSTYLE) }));
    let mut child = unsafe { GetWindow(progman, GW_CHILD) }.ok();
    while let Some(c) = child.filter(|c| !c.is_invalid()) {
        let mut r = RECT::default();
        unsafe {
            let _ = GetWindowRect(c, &mut r);
        }
        s.push_str(&format!(
            "  enfant {:?} {} ex={:#x} {:?}\n",
            c.0,
            class_name(c),
            unsafe { GetWindowLongW(c, GWL_EXSTYLE) },
            (r.left, r.top, r.right, r.bottom)
        ));
        child = unsafe { GetWindow(c, GW_HWNDNEXT) }.ok();
    }
    s
}

//! Fil d'interface : icône de notification et menu, fenêtres hôtes, et
//! décision écran par écran d'animer ou non. Ce fil passe sa vie bloqué dans
//! `GetMessageW` ; un minuteur de 750 ms + quelques événements système
//! suffisent à suivre l'état du bureau.

use crate::config::{Config, FPS_CHOICES, SCALE_CHOICES, SPEED_CHOICES};
use crate::desktop::{self, Host, Monitor};
use crate::render::{self, Settings, Shared};
use crate::shaders::{self, Entry};
use crate::util::{self, WStr};
use crate::{autostart, log};
use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::{
    FindFirstChangeNotificationW, FindNextChangeNotification, FILE_NOTIFY_CHANGE_FILE_NAME,
    FILE_NOTIFY_CHANGE_LAST_WRITE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    GetSystemPowerStatus, RegisterPowerSettingNotification, POWERBROADCAST_SETTING, SYSTEM_POWER_STATUS,
};
use windows::Win32::System::ProcessStatus::K32EmptyWorkingSet;
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::System::SystemServices::GUID_CONSOLE_DISPLAY_STATE;
use windows::Win32::System::Threading::{GetCurrentProcess, WaitForSingleObject, INFINITE};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const TRAY_CLASS: &str = "OndeTray";
const WM_TRAY: u32 = WM_APP + 1;
const WM_WAKE: u32 = WM_APP + 2;
const WM_EVAL: u32 = WM_APP + 3;
const WM_SHADERS_CHANGED: u32 = WM_APP + 4;
/// Envoyé par `onde.exe --quitter`.
pub const WM_QUIT_REQUEST: u32 = WM_APP + 5;
pub const TRAY_CLASS_NAME: &str = TRAY_CLASS;

const TIMER_EVAL: usize = 1;
const TIMER_RELOAD: usize = 2;
const TIMER_REATTACH: usize = 3;
const TIMER_FORCE_QUIT: usize = 4;

const ID_FOND: usize = 1000;
const ID_FPS: usize = 300;
const ID_SCALE: usize = 320;
const ID_SPEED: usize = 340;
const ID_SON: usize = 400;
const ID_SOURIS: usize = 401;
const ID_PAUSE: usize = 402;
const ID_PAUSE_BATT: usize = 403;
const ID_PAUSE_HORS: usize = 404;
const ID_DOSSIER: usize = 405;
const ID_AUTOSTART: usize = 406;
const ID_JOURNAL: usize = 407;
const ID_QUIT: usize = 408;

struct App {
    hwnd: HWND,
    icon: HICON,
    taskbar_created: u32,
    cfg: Config,
    shared: Arc<Shared>,
    host: Option<Host>,
    mons: Vec<Monitor>,
    /// (génération, fenêtre hôte) : détruites quand le rendu les a lâchées.
    hosts: Vec<(u64, isize)>,
    gen: u64,
    shader_gen: u64,
    settings_gen: u64,
    active: Vec<bool>,
    reason: String,
    locked: bool,
    display_off: bool,
    entries: Vec<Entry>,
    quitting: bool,
    tip: String,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

static UI_HWND: AtomicIsize = AtomicIsize::new(0);

fn with<R>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with(|a| f(a.borrow_mut().as_mut().expect("application non initialisée")))
}

fn settings_of(c: &Config) -> Settings {
    Settings {
        fps: c.fps,
        scale: c.echelle as f32 / 100.0,
        speed: c.vitesse as f32 / 100.0,
        audio: c.son,
        mouse: c.souris,
    }
}

pub fn run() {
    // Une seule instance par session : une seconde la réveille simplement.
    let mutex = unsafe { windows::Win32::System::Threading::CreateMutexW(None, true, w!("Local\\Onde.Resident")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        if let Ok(h) = unsafe { FindWindowW(WStr::new(TRAY_CLASS).pcwstr(), PCWSTR::null()) } {
            unsafe {
                let _ = PostMessageW(Some(h), WM_WAKE, WPARAM(0), LPARAM(0));
            }
        }
        return;
    }

    util::init_log();
    log!("Onde {} démarré", env!("CARGO_PKG_VERSION"));
    crate::efficiency();
    shaders::ensure_user_dir();
    let cfg = Config::load();
    let shared = Shared::new(settings_of(&cfg));

    unsafe {
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let class = WStr::new(TRAY_CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class.pcwstr(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        desktop::register_class();
        // Fenêtre cachée de premier niveau (pas « message-only ») : seules
        // celles-ci reçoivent TaskbarCreated et les notifications d'alimentation.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class.pcwstr(),
            w!("Onde"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance),
            None,
        )
        .expect("fenêtre d'interface");
        UI_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

        let icon = LoadImageW(
            Some(hinstance),
            PCWSTR(1 as _),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        )
        .map(|h| HICON(h.0))
        .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap_or_default());

        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                hwnd,
                icon,
                taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
                cfg,
                shared: shared.clone(),
                host: None,
                mons: Vec::new(),
                hosts: Vec::new(),
                gen: 0,
                shader_gen: 0,
                settings_gen: 0,
                active: Vec::new(),
                reason: String::new(),
                locked: false,
                display_off: false,
                entries: Vec::new(),
                quitting: false,
                tip: String::new(),
            })
        });

        let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        let _ =
            RegisterPowerSettingNotification(HANDLE(hwnd.0), &GUID_CONSOLE_DISPLAY_STATE, DEVICE_NOTIFY_WINDOW_HANDLE);
        // Premier plan, réduction/restauration, fin de déplacement : réaction immédiate.
        // WINEVENT_OUTOFCONTEXT (0) | WINEVENT_SKIPOWNPROCESS (2)
        SetWinEventHook(0x0003, 0x0017, None, Some(win_event), 0, 0, 0x0002);
        SetTimer(Some(hwnd), TIMER_EVAL, 750, None);

        add_icon();
        watch_user_dir(hwnd);
        load_shader();
        attach();

        let ui = hwnd.0 as isize;
        let render_shared = shared.clone();
        std::thread::Builder::new()
            .name("rendu".into())
            .spawn(move || render::run(render_shared, ui))
            .expect("fil de rendu");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if let Ok(m) = mutex {
            let _ = CloseHandle(m);
        }
    }
    log!("Onde arrêté");
}

unsafe extern "system" fn win_event(_: HWINEVENTHOOK, event: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
    // EVENT_SYSTEM_FOREGROUND, MOVESIZEEND, MINIMIZESTART, MINIMIZEEND
    if matches!(event, 0x0003 | 0x000B | 0x0016 | 0x0017) {
        let h = UI_HWND.load(Ordering::Relaxed);
        let _ = PostMessageW(Some(HWND(h as *mut _)), WM_EVAL, WPARAM(0), LPARAM(0));
    }
}

/// (Re)crée les fenêtres hôtes sous les icônes, un par écran.
fn attach() {
    let host = desktop::find_host();
    let mons = desktop::monitors();
    let (descs, gen) = with(|a| {
        a.gen += 1;
        let descs = match &host {
            Some(h) => desktop::create_hosts(h, &mons),
            None => Vec::new(),
        };
        for d in &descs {
            a.hosts.push((a.gen, d.hwnd));
        }
        a.host = host;
        a.mons = mons.clone();
        a.active.clear();
        (descs, a.gen)
    });
    match &host {
        Some(h) => log!(
            "bureau : parent {} {:?}, {} écran(s){}",
            util::class_name(h.parent),
            h.parent.0,
            descs.len(),
            if h.defview.is_some() { " (disposition 24H2)" } else { "" }
        ),
        None => log!("bureau introuvable (Explorateur arrêté ?)"),
    }
    let shared = with(|a| a.shared.clone());
    shared.update(|c| {
        c.surfaces = descs;
        c.surfaces_gen = gen;
        c.active = Vec::new();
    });
    evaluate();
}

fn on_battery() -> bool {
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s) }.is_ok() && (s.ACLineStatus == 0 || s.SystemStatusFlag == 1)
}

fn exclusive_fullscreen() -> bool {
    matches!(unsafe { SHQueryUserNotificationState() }, Ok(QUNS_RUNNING_D3D_FULL_SCREEN) | Ok(QUNS_PRESENTATION_MODE))
}

/// Décide, écran par écran, s'il faut animer. Appelé toutes les 750 ms et
/// à chaque changement de fenêtre au premier plan.
fn evaluate() {
    let (cfg, locked, display_off, mons) = with(|a| (a.cfg.clone(), a.locked, a.display_off, a.mons.clone()));
    let global = if cfg.pause {
        Some("en pause")
    } else if locked {
        Some("session verrouillée")
    } else if display_off {
        Some("écran éteint")
    } else if cfg.pause_batterie && on_battery() {
        Some("pause sur batterie")
    } else if exclusive_fullscreen() {
        Some("jeu en plein écran")
    } else if cfg.pause_hors_bureau && !desktop::desktop_in_front() {
        Some("application au premier plan")
    } else {
        None
    };
    let (active, reason) = match global {
        Some(r) => (vec![false; mons.len()], r.to_string()),
        None => {
            let covered = desktop::covered_monitors(&mons);
            let active: Vec<bool> = covered.iter().map(|c| !c).collect();
            let n = active.iter().filter(|a| **a).count();
            let reason = if n == 0 && !mons.is_empty() {
                "écrans masqués par des fenêtres".to_string()
            } else if n < mons.len() {
                format!("animé sur {n} écran(s) sur {}", mons.len())
            } else {
                "animé".to_string()
            };
            (active, reason)
        }
    };
    let changed = with(|a| {
        let changed = a.active != active;
        a.active = active.clone();
        a.reason = reason;
        changed
    });
    if changed {
        let all_paused = !active.iter().any(|x| *x);
        let shared = with(|a| a.shared.clone());
        shared.update(|c| c.active = active);
        if all_paused {
            // Rendre la mémoire inutilisée pendant la pause.
            unsafe {
                let _ = K32EmptyWorkingSet(GetCurrentProcess());
            }
        }
    }
    update_tip();
}

fn load_shader() {
    let (id, entries) = with(|a| (a.cfg.fond.clone(), shaders::list()));
    let (file, src) = match shaders::source(&id) {
        Some(src) => (id.trim_start_matches("perso:").to_string(), src),
        None => {
            log!("fond « {id} » introuvable, retour au fond par défaut");
            let b = &shaders::BUILTINS[0];
            with(|a| a.cfg.fond = b.id.to_string());
            (b.id.to_string(), b.source.to_string())
        }
    };
    let (shared, gen) = with(|a| {
        a.entries = entries;
        a.shader_gen += 1;
        (a.shared.clone(), a.shader_gen)
    });
    shared.update(|c| {
        c.shader = Some((file, src));
        c.shader_gen = gen;
    });
}

fn push_settings() {
    let (shared, gen, s) = with(|a| {
        a.settings_gen += 1;
        (a.shared.clone(), a.settings_gen, settings_of(&a.cfg))
    });
    shared.update(|c| {
        c.settings = s;
        c.settings_gen = gen;
    });
}

/// Surveille le dossier des fonds personnels pour recompiler à chaque enregistrement.
fn watch_user_dir(hwnd: HWND) {
    let dir = util::user_shader_dir();
    let hwnd = hwnd.0 as isize;
    std::thread::spawn(move || unsafe {
        let Ok(h) = FindFirstChangeNotificationW(
            WStr::new(&dir).pcwstr(),
            false,
            FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_FILE_NAME,
        ) else {
            return;
        };
        while WaitForSingleObject(h, INFINITE) == WAIT_OBJECT_0 {
            let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_SHADERS_CHANGED, WPARAM(0), LPARAM(0));
            if FindNextChangeNotification(h).is_err() {
                break;
            }
        }
    });
}

// ---------------------------------------------------------------- icône

fn icon_data() -> NOTIFYICONDATAW {
    with(|a| NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: a.hwnd,
        uID: 1,
        ..Default::default()
    })
}

fn copy_wide(dst: &mut [u16], text: &str) {
    let src: Vec<u16> = text.encode_utf16().take(dst.len() - 1).collect();
    dst[..src.len()].copy_from_slice(&src);
    dst[src.len()] = 0;
}

fn current_name() -> String {
    with(|a| {
        let entries = if a.entries.is_empty() { shaders::list() } else { a.entries.clone() };
        entries.iter().find(|e| e.id == a.cfg.fond).map(|e| e.name.clone()).unwrap_or_else(|| a.cfg.fond.clone())
    })
}

fn tooltip() -> String {
    let name = current_name();
    with(|a| format!("Onde — {name}\n{}", a.reason))
}

fn add_icon() {
    let mut nid = icon_data();
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = with(|a| a.icon);
    let tip = tooltip();
    copy_wide(&mut nid.szTip, &tip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
    with(|a| a.tip = tip);
}

fn update_tip() {
    let tip = tooltip();
    if with(|a| a.tip == tip) {
        return;
    }
    let mut nid = icon_data();
    nid.uFlags = NIF_TIP;
    copy_wide(&mut nid.szTip, &tip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
    with(|a| a.tip = tip);
}

fn notify(title: &str, text: &str, warning: bool) {
    let mut nid = icon_data();
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = if warning { NIIF_WARNING } else { NIIF_INFO };
    copy_wide(&mut nid.szInfoTitle, title);
    copy_wide(&mut nid.szInfo, text);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

// ---------------------------------------------------------------- menu

fn change(f: impl FnOnce(&mut Config)) {
    let r = with(|a| {
        f(&mut a.cfg);
        a.cfg.save()
    });
    if let Err(e) = r {
        notify("Onde", &e, true);
    }
}

unsafe fn append(menu: HMENU, flags: MENU_ITEM_FLAGS, id: usize, text: &str) {
    let _ = AppendMenuW(menu, flags, id, WStr::new(text).pcwstr());
}

fn check(on: bool) -> MENU_ITEM_FLAGS {
    if on {
        MF_CHECKED
    } else {
        MF_UNCHECKED
    }
}

fn open(path: &std::path::Path, with_notepad: bool) {
    unsafe {
        if with_notepad {
            ShellExecuteW(
                None,
                w!("open"),
                w!("notepad.exe"),
                WStr::new(format!("\"{}\"", path.display())).pcwstr(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        } else {
            ShellExecuteW(None, w!("open"), WStr::new(path).pcwstr(), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
        }
    }
}

fn show_menu() {
    let entries = shaders::list();
    let (hwnd, cfg, reason) = with(|a| {
        a.entries = entries.clone();
        (a.hwnd, a.cfg.clone(), a.reason.clone())
    });
    let autostart_on = autostart::is_enabled();
    unsafe {
        let menu = CreatePopupMenu().unwrap();
        let radio = MENU_ITEM_FLAGS(MFT_RADIOCHECK.0) | MF_STRING;

        append(menu, MF_STRING | MF_GRAYED, 0, &format!("Onde — {reason}"));
        append(menu, MF_SEPARATOR, 0, "");

        let fonds = CreatePopupMenu().unwrap();
        let mut user_started = false;
        for (i, e) in entries.iter().enumerate() {
            if e.path.is_some() && !user_started {
                user_started = true;
                append(fonds, MF_SEPARATOR, 0, "");
            }
            append(fonds, radio | check(e.id == cfg.fond), ID_FOND + i, &e.name);
        }
        append(fonds, MF_SEPARATOR, 0, "");
        append(fonds, MF_STRING, ID_DOSSIER, "Ouvrir le dossier des fonds personnels…");
        append(menu, MF_POPUP, fonds.0 as usize, "Fond d'écran");

        let fps = CreatePopupMenu().unwrap();
        for (i, v) in FPS_CHOICES.iter().enumerate() {
            append(fps, radio | check(*v == cfg.fps), ID_FPS + i, &format!("{v} images/s"));
        }
        append(menu, MF_POPUP, fps.0 as usize, &format!("Fluidité : {} images/s", cfg.fps));

        let scale = CreatePopupMenu().unwrap();
        for (i, v) in SCALE_CHOICES.iter().enumerate() {
            let note = match v {
                100 => " (le plus net)",
                50 => " (conseillé)",
                25 => " (le plus léger)",
                _ => "",
            };
            append(scale, radio | check(*v == cfg.echelle), ID_SCALE + i, &format!("{v} %{note}"));
        }
        append(menu, MF_POPUP, scale.0 as usize, &format!("Résolution de rendu : {} %", cfg.echelle));

        let speed = CreatePopupMenu().unwrap();
        for (i, v) in SPEED_CHOICES.iter().enumerate() {
            append(speed, radio | check(*v == cfg.vitesse), ID_SPEED + i, &format!("{v} %"));
        }
        append(menu, MF_POPUP, speed.0 as usize, &format!("Vitesse d'animation : {} %", cfg.vitesse));

        append(menu, MF_SEPARATOR, 0, "");
        append(menu, MF_STRING | check(cfg.souris), ID_SOURIS, "Réagir à la souris et aux clics");
        append(menu, MF_STRING | check(cfg.son), ID_SON, "Réagir au son");
        append(menu, MF_SEPARATOR, 0, "");
        append(menu, MF_STRING | check(cfg.pause_batterie), ID_PAUSE_BATT, "Pause sur batterie et en économiseur");
        append(
            menu,
            MF_STRING | check(cfg.pause_hors_bureau),
            ID_PAUSE_HORS,
            "Animer seulement quand le bureau est au premier plan",
        );
        append(menu, MF_STRING | check(cfg.pause), ID_PAUSE, "Mettre en pause");
        append(menu, MF_SEPARATOR, 0, "");
        append(menu, MF_STRING | check(autostart_on), ID_AUTOSTART, "Lancer au démarrage de Windows");
        append(menu, MF_STRING, ID_JOURNAL, "Ouvrir le journal");
        append(menu, MF_STRING, ID_QUIT, "Quitter");

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, None, hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        on_command(cmd.0 as usize, autostart_on);
    }
}

fn on_command(cmd: usize, autostart_on: bool) {
    let n_entries = with(|a| a.entries.len());
    match cmd {
        0 => {}
        c if (ID_FOND..ID_FOND + n_entries).contains(&c) => {
            let id = with(|a| a.entries[c - ID_FOND].id.clone());
            change(|cfg| cfg.fond = id);
            load_shader();
        }
        c if (ID_FPS..ID_FPS + FPS_CHOICES.len()).contains(&c) => {
            change(|cfg| cfg.fps = FPS_CHOICES[c - ID_FPS]);
            push_settings();
        }
        c if (ID_SCALE..ID_SCALE + SCALE_CHOICES.len()).contains(&c) => {
            change(|cfg| cfg.echelle = SCALE_CHOICES[c - ID_SCALE]);
            push_settings();
        }
        c if (ID_SPEED..ID_SPEED + SPEED_CHOICES.len()).contains(&c) => {
            change(|cfg| cfg.vitesse = SPEED_CHOICES[c - ID_SPEED]);
            push_settings();
        }
        ID_SON => {
            change(|cfg| cfg.son = !cfg.son);
            push_settings();
        }
        ID_SOURIS => {
            change(|cfg| cfg.souris = !cfg.souris);
            push_settings();
        }
        ID_PAUSE => {
            change(|cfg| cfg.pause = !cfg.pause);
            evaluate();
        }
        ID_PAUSE_BATT => {
            change(|cfg| cfg.pause_batterie = !cfg.pause_batterie);
            evaluate();
        }
        ID_PAUSE_HORS => {
            change(|cfg| cfg.pause_hors_bureau = !cfg.pause_hors_bureau);
            evaluate();
        }
        ID_DOSSIER => {
            shaders::ensure_user_dir();
            open(&util::user_shader_dir(), false);
        }
        ID_AUTOSTART => {
            let r = if autostart_on { autostart::disable() } else { autostart::enable() };
            if let Err(e) = r {
                notify("Lancement au démarrage", &e, true);
            }
        }
        ID_JOURNAL => open(&util::log_path(), true),
        ID_QUIT => quit(),
        _ => {}
    }
    update_tip();
}

fn quit() {
    let (shared, hwnd, already) = with(|a| (a.shared.clone(), a.hwnd, std::mem::replace(&mut a.quitting, true)));
    if already {
        return;
    }
    // Filet de sécurité si le rendu ne répond pas.
    unsafe { SetTimer(Some(hwnd), TIMER_FORCE_QUIT, 3000, None) };
    // Le rendu libère ses ressources puis répond WM_RENDER_DONE.
    shared.update(|c| c.quit = true);
}

fn finish() {
    let (hosts, host) = with(|a| (a.hosts.drain(..).map(|(_, h)| h).collect::<Vec<_>>(), a.host));
    desktop::destroy_hosts(&hosts);
    if let Some(h) = host {
        desktop::repaint(&h);
    }
    let nid = icon_data();
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        PostQuitMessage(0);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP || event == WM_LBUTTONUP || event == WM_CONTEXTMENU {
                show_menu();
            }
            LRESULT(0)
        }
        WM_TIMER => {
            match wparam.0 {
                TIMER_EVAL => evaluate(),
                TIMER_RELOAD => {
                    let _ = KillTimer(Some(hwnd), TIMER_RELOAD);
                    let entries = shaders::list();
                    let perso = with(|a| {
                        a.entries = entries;
                        a.cfg.fond.starts_with("perso:")
                    });
                    if perso {
                        load_shader();
                    }
                }
                TIMER_FORCE_QUIT => finish(),
                TIMER_REATTACH => {
                    let _ = KillTimer(Some(hwnd), TIMER_REATTACH);
                    attach();
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_EVAL => {
            evaluate();
            LRESULT(0)
        }
        WM_SHADERS_CHANGED => {
            SetTimer(Some(hwnd), TIMER_RELOAD, 300, None);
            LRESULT(0)
        }
        WM_QUIT_REQUEST => {
            quit();
            LRESULT(0)
        }
        WM_WAKE => {
            notify("Onde", "Onde est déjà lancé : réglages dans l'icône de la zone de notification.", false);
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            SetTimer(Some(hwnd), TIMER_REATTACH, 1500, None);
            LRESULT(0)
        }
        WM_WTSSESSION_CHANGE => {
            match wparam.0 as u32 {
                WTS_SESSION_LOCK => with(|a| a.locked = true),
                WTS_SESSION_UNLOCK => with(|a| a.locked = false),
                _ => {}
            }
            evaluate();
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            match wparam.0 as u32 {
                PBT_POWERSETTINGCHANGE => {
                    let s = &*(lparam.0 as *const POWERBROADCAST_SETTING);
                    if s.PowerSetting == GUID_CONSOLE_DISPLAY_STATE {
                        with(|a| a.display_off = s.Data[0] == 0);
                        evaluate();
                    }
                }
                PBT_APMPOWERSTATUSCHANGE => evaluate(),
                PBT_APMRESUMEAUTOMATIC => {
                    SetTimer(Some(hwnd), TIMER_REATTACH, 3000, None);
                }
                _ => {}
            }
            LRESULT(1)
        }
        render::WM_RENDER_ERROR => {
            let text = if lparam.0 != 0 { *Box::from_raw(lparam.0 as *mut String) } else { String::new() };
            if wparam.0 == 1 {
                // Erreur de compilation : on montre les premières lignes.
                let short: String = text.lines().take(3).collect::<Vec<_>>().join("\n");
                notify("Erreur dans le fond d'écran", &short, true);
            } else {
                notify("Onde : carte graphique indisponible", &text, true);
            }
            LRESULT(0)
        }
        render::WM_SHADER_OK => LRESULT(0),
        render::WM_SURFACES_RELEASED => {
            let gen = wparam.0 as u64;
            let old: Vec<isize> = with(|a| {
                let (old, keep): (Vec<_>, Vec<_>) = a.hosts.drain(..).partition(|(g, _)| *g < gen);
                a.hosts = keep;
                old.into_iter().map(|(_, h)| h).collect()
            });
            desktop::destroy_hosts(&old);
            LRESULT(0)
        }
        render::WM_RENDER_DONE => {
            finish();
            LRESULT(0)
        }
        _ => {
            let taskbar_created = APP.with(|a| a.borrow().as_ref().map(|a| a.taskbar_created));
            if taskbar_created == Some(msg) {
                // L'Explorateur a redémarré : nouvelle icône, nouveau bureau.
                log!("Explorateur redémarré");
                add_icon();
                SetTimer(Some(hwnd), TIMER_REATTACH, 1000, None);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

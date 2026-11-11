//! Petits utilitaires Win32, dossier de données et journal.

use std::ffi::OsStr;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

/// Chaîne UTF-16 terminée par NUL pour les appels Win32.
pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// Garde la chaîne vivante le temps de l'appel.
pub struct WStr(Vec<u16>);
impl WStr {
    pub fn new(s: impl AsRef<OsStr>) -> Self {
        WStr(wide(s))
    }
    pub fn pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// `%APPDATA%\Onde`
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("Onde")
}

/// Dossier des fonds personnalisés de l'utilisateur.
pub fn user_shader_dir() -> PathBuf {
    data_dir().join("fonds")
}

static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn log_path() -> PathBuf {
    data_dir().join("onde.log")
}

pub fn init_log() {
    let _ = std::fs::create_dir_all(data_dir());
    if let Ok(mut file) = std::fs::File::create(log_path()) {
        let _ = file.write_all("\u{feff}".as_bytes());
        *LOG.lock().unwrap() = Some(file);
    }
}

pub fn log(msg: &str) {
    let mut guard = LOG.lock().unwrap();
    if let Some(file) = guard.as_mut() {
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        let _ = writeln!(file, "{:02}:{:02}:{:02}  {msg}", t.wHour, t.wMinute, t.wSecond);
    } else {
        eprintln!("{msg}");
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => { $crate::util::log(&format!($($arg)*)) };
}

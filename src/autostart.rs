//! Lancement à l'ouverture de session : valeur `Onde` sous
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (pas besoin d'être admin).

use windows::core::w;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::*;

const RUN: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

pub fn is_enabled() -> bool {
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN, w!("Onde"), RRF_RT_REG_SZ, None, None, None) == ERROR_SUCCESS }
}

pub fn enable() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let value: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain(std::iter::once(0)).collect();
    let r = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            RUN,
            w!("Onde"),
            REG_SZ.0,
            Some(value.as_ptr() as *const _),
            (value.len() * 2) as u32,
        )
    };
    if r == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("écriture dans le registre impossible ({})", r.0))
    }
}

pub fn disable() -> Result<(), String> {
    let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, w!("Onde")) };
    if r == ERROR_SUCCESS || r.0 == 2 {
        Ok(())
    } else {
        Err(format!("suppression dans le registre impossible ({})", r.0))
    }
}

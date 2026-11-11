//! Onde — fonds d'écran animés et réactifs qui ne consomment quasiment rien.

#![windows_subsystem = "windows"]

mod app;
mod audio;
mod autostart;
mod config;
mod desktop;
mod render;
mod shaders;
mod util;

use windows::Win32::System::Threading::*;

/// Mode « efficacité » de Windows 11 (EcoQoS) et priorité basse : le peu de
/// CPU utilisé l'est sur les cœurs économes, et tout le reste passe devant.
pub fn efficiency() {
    if std::env::var_os("ONDE_SANS_ECO").is_some() {
        return;
    }
    unsafe {
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        };
        let _ = SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            &state as *const _ as *const _,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
        let _ = SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

fn console() {
    unsafe {
        let _ = windows::Win32::System::Console::AttachConsole(u32::MAX);
    }
}

fn verify(args: &[String]) -> ! {
    let mut sources: Vec<(String, String)> =
        shaders::BUILTINS.iter().map(|b| (b.id.to_string(), b.source.to_string())).collect();
    sources.push(("exemple".into(), shaders::EXAMPLE.to_string()));
    for path in args {
        sources.push((path.clone(), std::fs::read_to_string(path).unwrap_or_default()));
    }
    let mut gpu = match render::Gpu::new() {
        Ok(g) => g,
        Err(e) => {
            println!("{e}");
            std::process::exit(2);
        }
    };
    let mut ok = true;
    for (name, src) in sources {
        match gpu.set_shader(&name, &src) {
            Ok(()) => println!("ok      {name}"),
            Err(e) => {
                ok = false;
                println!("ERREUR  {name}\n{e}");
            }
        }
    }
    std::process::exit(if ok { 0 } else { 1 });
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--verifier") => {
            console();
            verify(&args[1..]);
        }
        Some("--capture") => {
            // --capture <fond|fichier.hlsl> <sortie.bmp> [largeur] [hauteur] [temps]
            console();
            let id = args.get(1).cloned().unwrap_or_else(|| "aurore".into());
            let out = args.get(2).cloned().unwrap_or_else(|| "capture.bmp".into());
            let w = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(960);
            let h = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(540);
            let t = args.get(5).and_then(|v| v.parse().ok()).unwrap_or(12.0);
            let src = shaders::source(&id).or_else(|| std::fs::read_to_string(&id).ok()).unwrap_or_default();
            if let Err(e) = render::capture(&src, &id, w, h, t, std::path::Path::new(&out)) {
                println!("{e}");
                std::process::exit(1);
            }
        }
        Some("--quitter") => unsafe {
            use crate::util::WStr;
            use windows::core::PCWSTR;
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};
            if let Ok(h) = FindWindowW(WStr::new(app::TRAY_CLASS_NAME).pcwstr(), PCWSTR::null()) {
                let _ = PostMessageW(Some(h), app::WM_QUIT_REQUEST, WPARAM(0), LPARAM(0));
            }
        },
        Some("--diag") => {
            console();
            print!("{}", desktop::describe());
            let mons = desktop::monitors();
            for m in &mons {
                println!(
                    "écran {:?} travail {:?} {} Hz",
                    (m.rect.left, m.rect.top, m.rect.right, m.rect.bottom),
                    (m.work.left, m.work.top, m.work.right, m.work.bottom),
                    m.refresh
                );
            }
            println!("masqués : {:?}", desktop::covered_monitors(&mons));
        }
        _ => app::run(),
    }
}

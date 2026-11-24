//! Le fond sur l'écran de verrouillage.
//!
//! Windows n'autorise aucune application à dessiner sur l'écran de
//! verrouillage (il appartient à LockApp, sur un bureau protégé) : on y
//! place donc une image fixe tirée du fond, via l'API WinRT
//! `Windows.System.UserProfile.LockScreen`, qui fonctionne sans droits
//! administrateur depuis une application de bureau. L'image d'origine est
//! sauvegardée la première fois et remise quand l'option est désactivée.

use crate::util::{data_dir, WStr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{Interface, HSTRING};
use windows::Storage::StorageFile;
use windows::Storage::Streams::DataReader;
use windows::System::UserProfile::LockScreen;
use windows::Win32::Foundation::GENERIC_WRITE;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

static BUSY: AtomicBool = AtomicBool::new(false);

fn backup_marker() -> PathBuf {
    data_dir().join("verrouillage-origine.txt")
}

/// Met à jour l'image de l'écran de verrouillage en arrière-plan.
/// Une seule mise à jour à la fois ; les demandes pendant ce temps sont ignorées.
pub fn update(id: String, source: String, w: u32, h: u32) {
    if BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = std::thread::Builder::new().name("verrouillage".into()).spawn(move || {
        // Sur certains Windows 11 (26H2, build 26300), l'appel n'aboutit jamais
        // depuis une application non empaquetée : délai dépassé (0x800705B4)
        // après 30 s. Pas de nouvel essai rapproché : le minuteur horaire de
        // l'interface s'en charge (voir docs/ECRAN_DE_VERROUILLAGE.md).
        let t0 = std::time::Instant::now();
        match apply(&id, &source, w, h) {
            Ok(path) => crate::log!(
                "écran de verrouillage : image de « {id} » {w}×{h} appliquée en {} ms ({})",
                t0.elapsed().as_millis(),
                path.display()
            ),
            Err(e) => crate::log!(
                "écran de verrouillage : échec ({e}){}",
                if e.contains("0x800705B4") {
                    " — Windows ne répond pas à la demande (voir docs/ECRAN_DE_VERROUILLAGE.md)"
                } else {
                    ""
                }
            ),
        }
        BUSY.store(false, Ordering::Release);
    });
}

/// Remet l'image d'origine de l'utilisateur (sauvegardée avant la première modification).
pub fn restore() {
    let _ = std::thread::Builder::new().name("verrouillage".into()).spawn(|| {
        let r = (|| -> Result<(), String> {
            init_com();
            let saved = std::fs::read_to_string(backup_marker()).map_err(|_| "aucune sauvegarde".to_string())?;
            let path = PathBuf::from(saved.trim());
            set_image(&path)?;
            let _ = std::fs::remove_file(backup_marker());
            Ok(())
        })();
        match r {
            Ok(()) => crate::log!("écran de verrouillage : image d'origine remise"),
            Err(e) => crate::log!("écran de verrouillage : restauration impossible ({e})"),
        }
    });
}

fn init_com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn apply(id: &str, source: &str, w: u32, h: u32) -> Result<PathBuf, String> {
    init_com();
    backup_original()?;
    // Moment de l'animation : varie d'une fois sur l'autre, reste reproductible.
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let time = 20.0 + (t.wMinute as f32 * 60.0 + t.wSecond as f32) % 900.0;
    let pixels = crate::render::snapshot(source, id, w, h, time, true)?;
    // Deux noms en alternance : Windows ne relit pas toujours un fichier de même nom.
    let a = data_dir().join("verrouillage-a.png");
    let b = data_dir().join("verrouillage-b.png");
    let path = if std::fs::metadata(&a).and_then(|m| m.modified()).ok()
        > std::fs::metadata(&b).and_then(|m| m.modified()).ok()
    {
        b
    } else {
        a
    };
    write_png(&path, w, h, &pixels)?;
    set_image(&path)?;
    Ok(path)
}

fn set_image(path: &Path) -> Result<(), String> {
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str()))
        .and_then(|op| op.join())
        .map_err(|e| format!("fichier {} : {e}", path.display()))?;
    LockScreen::SetImageFileAsync(&file).and_then(|op| op.join()).map_err(|e| format!("LockScreen : {e}"))
}

/// Copie l'image actuelle de l'écran de verrouillage, une seule fois.
fn backup_original() -> Result<(), String> {
    if backup_marker().exists() {
        return Ok(());
    }
    let stream = LockScreen::GetImageStream().map_err(|e| format!("lecture de l'image actuelle : {e}"))?;
    let size = stream.Size().map_err(|e| e.to_string())? as u32;
    let input = stream.GetInputStreamAt(0).map_err(|e| e.to_string())?;
    let reader = DataReader::CreateDataReader(&input).map_err(|e| e.to_string())?;
    reader.LoadAsync(size).and_then(|op| op.join()).map_err(|e| e.to_string())?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes).map_err(|e| e.to_string())?;
    let ext = match bytes.get(..4) {
        Some([0x89, b'P', b'N', b'G']) => "png",
        Some([b'B', b'M', ..]) => "bmp",
        _ => "jpg",
    };
    let _ = std::fs::create_dir_all(data_dir());
    let copy = data_dir().join(format!("verrouillage-origine.{ext}"));
    std::fs::write(&copy, &bytes).map_err(|e| format!("sauvegarde de l'image d'origine : {e}"))?;
    std::fs::write(backup_marker(), copy.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    crate::log!("écran de verrouillage : image d'origine sauvegardée ({})", copy.display());
    Ok(())
}

/// Encode des pixels BGRA (haut en bas) en PNG avec WIC.
fn write_png(path: &Path, w: u32, h: u32, bgra: &[u8]) -> Result<(), String> {
    let e = |e: windows::core::Error| format!("PNG : {e}");
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(e)?;
        let stream = factory.CreateStream().map_err(e)?;
        stream.InitializeFromFilename(WStr::new(path).pcwstr(), GENERIC_WRITE.0).map_err(e)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()).map_err(e)?;
        encoder
            .Initialize(&stream.cast::<windows::Win32::System::Com::IStream>().map_err(e)?, WICBitmapEncoderNoCache)
            .map_err(e)?;
        let mut frame = None;
        let mut props = None;
        encoder.CreateNewFrame(&mut frame, &mut props).map_err(e)?;
        let frame = frame.ok_or("PNG : pas d'image")?;
        frame.Initialize(props.as_ref()).map_err(e)?;
        frame.SetSize(w, h).map_err(e)?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&mut format).map_err(e)?;
        if format != GUID_WICPixelFormat32bppBGRA {
            return Err("PNG : format de pixels refusé".into());
        }
        frame.WritePixels(h, w * 4, bgra).map_err(e)?;
        frame.Commit().map_err(e)?;
        encoder.Commit().map_err(e)?;
    }
    Ok(())
}

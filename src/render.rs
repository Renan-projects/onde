//! Moteur de rendu Direct3D 11, sur son propre fil d'exécution.
//!
//! Principe d'économie : un seul triangle plein écran par image, dessiné à
//! résolution réduite puis étiré par le compositeur (gratuit) ; cadence
//! calée sur la synchro verticale (`Present(n)` + objet d'attente DXGI, donc
//! aucune attente active) ; et quand aucun écran n'est visible, le fil dort
//! sur un événement — 0 % de CPU et de GPU.

use crate::audio::Audio;
use crate::shaders;
use crate::util::class_name;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::core::{s, Interface, HRESULT, PCSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::Fxc::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{WaitForMultipleObjects, WaitForSingleObject, INFINITE};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, PostMessageW, WindowFromPoint, WM_APP};

pub const WM_RENDER_ERROR: u32 = WM_APP + 10;
pub const WM_SURFACES_RELEASED: u32 = WM_APP + 11;
pub const WM_RENDER_DONE: u32 = WM_APP + 12;
pub const WM_SHADER_OK: u32 = WM_APP + 13;

/// Un écran à animer : fenêtre hôte (créée par le fil d'interface) et
/// rectangle du moniteur en coordonnées écran.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceDesc {
    pub hwnd: isize,
    pub rect: RECT,
    pub refresh: u32,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub fps: u32,
    pub scale: f32,
    pub speed: f32,
    pub audio: bool,
    pub mouse: bool,
}

/// État partagé entre l'interface (qui écrit) et le rendu (qui lit).
pub struct Ctl {
    pub surfaces: Vec<SurfaceDesc>,
    pub surfaces_gen: u64,
    /// Écran par écran : faut-il animer ? (fenêtre plein écran, verrouillage…)
    pub active: Vec<bool>,
    /// (nom du fichier pour les erreurs, source HLSL)
    pub shader: Option<(String, String)>,
    pub shader_gen: u64,
    pub settings: Settings,
    pub settings_gen: u64,
    pub quit: bool,
}

pub struct Shared {
    pub ctl: Mutex<Ctl>,
    /// Événement auto-réinitialisé qui réveille le fil de rendu.
    wake: isize,
}

impl Shared {
    pub fn new(settings: Settings) -> Arc<Shared> {
        let wake =
            unsafe { windows::Win32::System::Threading::CreateEventW(None, false, false, None) }.expect("CreateEventW");
        Arc::new(Shared {
            ctl: Mutex::new(Ctl {
                surfaces: Vec::new(),
                surfaces_gen: 0,
                active: Vec::new(),
                shader: None,
                shader_gen: 0,
                settings,
                settings_gen: 0,
                quit: false,
            }),
            wake: wake.0 as isize,
        })
    }

    fn wake_handle(&self) -> HANDLE {
        HANDLE(self.wake as *mut _)
    }

    /// Modifie l'état partagé puis réveille le rendu.
    pub fn update(&self, f: impl FnOnce(&mut Ctl)) {
        f(&mut self.ctl.lock().unwrap());
        unsafe {
            let _ = windows::Win32::System::Threading::SetEvent(self.wake_handle());
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Constants {
    resolution: [f32; 3],
    time: f32,
    mouse: [f32; 4],
    son: [f32; 4],
    clics: [[f32; 4]; 4],
    souris: [f32; 2],
    presence: f32,
    heure: f32,
    ecran: f32,
    dt: f32,
    frame: f32,
    pad: f32,
}

pub struct Gpu {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    factory: IDXGIFactory2,
    vs: ID3D11VertexShader,
    ps: Option<ID3D11PixelShader>,
    cb: ID3D11Buffer,
    ps_target: PCSTR,
}

fn compile(src: &str, file: &str, entry: PCSTR, target: PCSTR) -> Result<Vec<u8>, String> {
    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    let name = std::ffi::CString::new(file.replace('\0', "")).unwrap_or_default();
    let r = unsafe {
        D3DCompile(
            src.as_ptr() as *const _,
            src.len(),
            PCSTR(name.as_ptr() as *const u8),
            None,
            None::<&ID3DInclude>,
            entry,
            target,
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    let blob_bytes = |b: &ID3DBlob| unsafe {
        std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize()).to_vec()
    };
    match (r, code) {
        (Ok(()), Some(code)) => Ok(blob_bytes(&code)),
        _ => {
            let msg = errors
                .map(|e| String::from_utf8_lossy(&blob_bytes(&e)).trim_end_matches('\0').trim().to_string())
                .unwrap_or_else(|| "échec de compilation".into());
            Err(msg)
        }
    }
}

impl Gpu {
    pub fn new() -> Result<Gpu, String> {
        unsafe {
            let levels = [D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0];
            let mut device = None;
            let mut ctx = None;
            let mut level = D3D_FEATURE_LEVEL::default();
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut level),
                Some(&mut ctx),
            )
            .map_err(|e| format!("Direct3D 11 indisponible : {e}"))?;
            let device: ID3D11Device = device.ok_or("pas de périphérique Direct3D")?;
            let ctx = ctx.ok_or("pas de contexte Direct3D")?;
            let dxgi: IDXGIDevice = device.cast().map_err(|e| e.to_string())?;
            // Priorité GPU minimale : un jeu lancé à côté passe toujours devant.
            let _ = dxgi.SetGPUThreadPriority(-7);
            let factory: IDXGIFactory2 =
                dxgi.GetAdapter().and_then(|a| a.GetParent()).map_err(|e| format!("DXGI : {e}"))?;

            let ps_target = if level.0 >= D3D_FEATURE_LEVEL_11_0.0 { s!("ps_5_0") } else { s!("ps_4_0") };
            let vs_target = if level.0 >= D3D_FEATURE_LEVEL_11_0.0 { s!("vs_5_0") } else { s!("vs_4_0") };
            let vs_code = compile(shaders::VERTEX, "onde-vs", s!("VS"), vs_target)?;
            let mut vs = None;
            device.CreateVertexShader(&vs_code, None, Some(&mut vs)).map_err(|e| e.to_string())?;

            let desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<Constants>() as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };
            let mut cb = None;
            device.CreateBuffer(&desc, None, Some(&mut cb)).map_err(|e| e.to_string())?;

            Ok(Gpu { device, ctx, factory, vs: vs.unwrap(), ps: None, cb: cb.unwrap(), ps_target })
        }
    }

    /// Compile un fond ; en cas d'erreur l'ancien reste en place.
    pub fn set_shader(&mut self, file: &str, source: &str) -> Result<(), String> {
        let full = shaders::pixel_source(source, file);
        let code = compile(&full, file, s!("PS"), self.ps_target)?;
        let mut ps = None;
        unsafe { self.device.CreatePixelShader(&code, None, Some(&mut ps)) }.map_err(|e| e.to_string())?;
        self.ps = ps;
        Ok(())
    }

    fn draw(&self, rtv: &ID3D11RenderTargetView, w: u32, h: u32, k: &Constants) -> Result<(), String> {
        let Some(ps) = &self.ps else { return Ok(()) };
        unsafe {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx.Map(&self.cb, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped)).map_err(|e| e.to_string())?;
            std::ptr::copy_nonoverlapping(k, mapped.pData as *mut Constants, 1);
            self.ctx.Unmap(&self.cb, 0);
            self.ctx.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            let vp = D3D11_VIEWPORT { Width: w as f32, Height: h as f32, MaxDepth: 1.0, ..Default::default() };
            self.ctx.RSSetViewports(Some(&[vp]));
            self.ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.ctx.VSSetShader(&self.vs, None);
            self.ctx.PSSetShader(ps, None);
            self.ctx.PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            self.ctx.Draw(3, 0);
        }
        Ok(())
    }

    /// Libère la mémoire temporaire du pilote pendant une pause.
    fn trim(&self) {
        unsafe {
            self.ctx.ClearState();
            self.ctx.Flush();
            if let Ok(d) = self.device.cast::<IDXGIDevice3>() {
                d.Trim();
            }
        }
    }
}

struct Surface {
    desc: SurfaceDesc,
    index: usize,
    swap: IDXGISwapChain1,
    rtv: ID3D11RenderTargetView,
    waitable: Option<HANDLE>,
    w: u32,
    h: u32,
    interval: u32,
    presence: f32,
    frames: u32,
}

impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(h) = self.waitable.take() {
            unsafe {
                let _ = CloseHandle(h);
            }
        }
    }
}

fn rect_size(r: &RECT) -> (i32, i32) {
    ((r.right - r.left).max(1), (r.bottom - r.top).max(1))
}

impl Surface {
    fn new(gpu: &Gpu, desc: &SurfaceDesc, index: usize, s: &Settings) -> Result<Surface, String> {
        let (rw, rh) = rect_size(&desc.rect);
        let w = ((rw as f32 * s.scale).round() as u32).max(16);
        let h = ((rh as f32 * s.scale).round() as u32).max(16);
        let hwnd = HWND(desc.hwnd as *mut _);
        let mut sd = DXGI_SWAP_CHAIN_DESC1 {
            Width: w,
            Height: h,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
            ..Default::default()
        };
        let (swap, waitable) = unsafe {
            match gpu.factory.CreateSwapChainForHwnd(&gpu.device, hwnd, &sd, None, None) {
                Ok(swap) => {
                    let waitable = swap.cast::<IDXGISwapChain2>().ok().and_then(|s2| {
                        s2.SetMaximumFrameLatency(1).ok()?;
                        let h = s2.GetFrameLatencyWaitableObject();
                        (!h.is_invalid()).then_some(h)
                    });
                    (swap, waitable)
                }
                Err(e) => {
                    // Ancien modèle « blit » : toujours disponible.
                    crate::log!("modèle flip refusé ({e}), repli sur le modèle classique");
                    sd.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;
                    sd.BufferCount = 1;
                    sd.Flags = 0;
                    let swap = gpu
                        .factory
                        .CreateSwapChainForHwnd(&gpu.device, hwnd, &sd, None, None)
                        .map_err(|e| format!("chaîne d'échange impossible : {e}"))?;
                    (swap, None)
                }
            }
        };
        unsafe {
            let _ = gpu.factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER | DXGI_MWA_NO_WINDOW_CHANGES);
        }
        let buffer: ID3D11Texture2D = unsafe { swap.GetBuffer(0) }.map_err(|e| e.to_string())?;
        let mut rtv = None;
        unsafe { gpu.device.CreateRenderTargetView(&buffer, None, Some(&mut rtv)) }.map_err(|e| e.to_string())?;
        let refresh = if desc.refresh > 1 { desc.refresh } else { 60 };
        let interval = ((refresh as f32 / s.fps.max(1) as f32).round() as u32).clamp(1, 4);
        crate::log!(
            "écran {index} : {rw}×{rh} → rendu {w}×{h}, {refresh} Hz / {interval} = {} i/s{}",
            refresh / interval,
            if waitable.is_some() { "" } else { " (sans objet d'attente)" }
        );
        Ok(Surface {
            desc: desc.clone(),
            index,
            swap,
            rtv: rtv.unwrap(),
            waitable,
            w,
            h,
            interval,
            presence: 0.0,
            frames: 0,
        })
    }
}

/// Souris et clics, lus par scrutation (aucun crochet système).
struct Input {
    pos: POINT,
    last_move: Instant,
    was_down: bool,
    /// (position écran, instant)
    clicks: [(POINT, Option<Instant>); 4],
    next_click: usize,
}

fn is_desktop_window(hwnd: HWND) -> bool {
    matches!(
        class_name(hwnd).as_str(),
        "SysListView32" | "SHELLDLL_DefView" | "WorkerW" | "Progman" | crate::desktop::HOST_CLASS
    )
}

impl Input {
    fn new() -> Input {
        Input {
            pos: POINT { x: -100000, y: -100000 },
            last_move: Instant::now() - std::time::Duration::from_secs(60),
            was_down: false,
            clicks: [(POINT::default(), None); 4],
            next_click: 0,
        }
    }

    fn poll(&mut self) {
        let mut p = POINT::default();
        if unsafe { GetCursorPos(&mut p) }.is_ok() && (p.x != self.pos.x || p.y != self.pos.y) {
            self.pos = p;
            self.last_move = Instant::now();
        }
        let state = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } as u16;
        let down = state & 0x8000 != 0;
        if (down && !self.was_down) || (!down && state & 1 != 0) {
            let target = unsafe { WindowFromPoint(self.pos) };
            if is_desktop_window(target) {
                self.clicks[self.next_click] = (self.pos, Some(Instant::now()));
                self.next_click = (self.next_click + 1) % 4;
            }
        }
        self.was_down = down;
    }
}

fn inside(r: &RECT, p: POINT) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}

/// Convertit un point écran en pixels de rendu (origine en bas à gauche).
fn to_local(s: &Surface, p: POINT) -> [f32; 2] {
    let (rw, rh) = rect_size(&s.desc.rect);
    [
        (p.x - s.desc.rect.left) as f32 * s.w as f32 / rw as f32,
        (s.desc.rect.bottom - p.y) as f32 * s.h as f32 / rh as f32,
    ]
}

fn local_hour() -> f32 {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    t.wHour as f32 + t.wMinute as f32 / 60.0 + t.wSecond as f32 / 3600.0
}

fn post(ui: isize, msg: u32, wparam: usize, text: Option<String>) {
    let lparam = text.map(|t| Box::into_raw(Box::new(t)) as isize).unwrap_or(0);
    unsafe {
        if PostMessageW(Some(HWND(ui as *mut _)), msg, WPARAM(wparam), LPARAM(lparam)).is_err() && lparam != 0 {
            drop(Box::from_raw(lparam as *mut String));
        }
    }
}

const DEVICE_LOST: [HRESULT; 3] =
    [DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET, DXGI_ERROR_DRIVER_INTERNAL_ERROR];

pub fn run(shared: Arc<Shared>, ui: isize) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let wake = shared.wake_handle();
    let mut gpu: Option<Gpu> = None;
    let mut surfaces: Vec<Surface> = Vec::new();
    let (mut seen_surfaces, mut seen_shader, mut seen_settings) = (u64::MAX, u64::MAX, u64::MAX);
    let mut settings = shared.ctl.lock().unwrap().settings.clone();
    let mut shader_audio = false;
    let mut audio: Option<Audio> = None;
    let mut audio_retry = Instant::now();
    let mut input = Input::new();
    let mut anim_time = 0.0f64;
    let mut last_tick: Option<Instant> = None;
    let mut idle = false;
    let mut stats = (Instant::now(), 0u32);
    // Temps CPU passé à préparer / dessiner / présenter (pour le journal).
    let mut cost = [0f64; 3];

    loop {
        // 1. Lire les changements demandés par l'interface.
        let (quit, new_surfaces, new_shader, new_settings, active) = {
            let c = shared.ctl.lock().unwrap();
            (
                c.quit,
                (c.surfaces_gen != seen_surfaces).then(|| (c.surfaces_gen, c.surfaces.clone())),
                (c.shader_gen != seen_shader).then(|| (c.shader_gen, c.shader.clone())),
                (c.settings_gen != seen_settings).then(|| (c.settings_gen, c.settings.clone())),
                c.active.clone(),
            )
        };
        if quit {
            break;
        }

        if gpu.is_none() {
            match Gpu::new() {
                Ok(g) => {
                    gpu = Some(g);
                    seen_shader = u64::MAX;
                    seen_surfaces = u64::MAX;
                    continue;
                }
                Err(e) => {
                    crate::log!("{e}");
                    post(ui, WM_RENDER_ERROR, 0, Some(e));
                    unsafe { WaitForSingleObject(wake, 5000) };
                    continue;
                }
            }
        }
        let g = gpu.as_mut().unwrap();

        let mut rebuild = false;
        if let Some((gen, s)) = new_settings {
            seen_settings = gen;
            if s.scale != settings.scale || s.fps != settings.fps {
                rebuild = true;
            }
            settings = s;
        }
        if let Some((gen, shader)) = new_shader {
            seen_shader = gen;
            if let Some((file, src)) = shader {
                match g.set_shader(&file, &src) {
                    Ok(()) => {
                        shader_audio = shaders::uses_audio(&src);
                        crate::log!("fond « {file} » compilé (son : {shader_audio})");
                        post(ui, WM_SHADER_OK, 0, None);
                    }
                    Err(e) => {
                        crate::log!("erreur de compilation de {file} :\n{e}");
                        post(ui, WM_RENDER_ERROR, 1, Some(e));
                    }
                }
            }
        }
        let mut surfaces_gen = None;
        if let Some((gen, descs)) = new_surfaces {
            seen_surfaces = gen;
            surfaces_gen = Some(gen);
            surfaces.clear();
            rebuild = false;
            for (i, d) in descs.iter().enumerate() {
                match Surface::new(g, d, i, &settings) {
                    Ok(s) => surfaces.push(s),
                    Err(e) => crate::log!("écran {i} : {e}"),
                }
            }
        }
        if rebuild {
            let descs: Vec<SurfaceDesc> = surfaces.iter().map(|s| s.desc.clone()).collect();
            surfaces.clear();
            for (i, d) in descs.iter().enumerate() {
                match Surface::new(g, d, i, &settings) {
                    Ok(s) => surfaces.push(s),
                    Err(e) => crate::log!("écran {i} : {e}"),
                }
            }
        }
        if let Some(gen) = surfaces_gen {
            // Les anciennes fenêtres hôtes ne sont plus utilisées : l'interface peut les détruire.
            post(ui, WM_SURFACES_RELEASED, gen as usize, None);
        }

        // 2. Écrans à animer.
        let live: Vec<usize> = (0..surfaces.len())
            .filter(|&i| g.ps.is_some() && active.get(surfaces[i].index).copied().unwrap_or(false))
            .collect();

        let want_audio = settings.audio && shader_audio && !live.is_empty();
        if !want_audio {
            audio = None;
        } else if audio.is_none() && Instant::now() >= audio_retry {
            match Audio::new() {
                Ok(a) => audio = Some(a),
                Err(e) => {
                    crate::log!("capture du son impossible : {e}");
                    audio_retry = Instant::now() + std::time::Duration::from_secs(10);
                }
            }
        }

        if live.is_empty() {
            if !idle {
                idle = true;
                last_tick = None;
                g.trim();
                crate::log!("en pause");
            }
            unsafe { WaitForSingleObject(wake, INFINITE) };
            continue;
        }
        if idle {
            idle = false;
            crate::log!("animation reprise");
        }

        // 3. Attendre qu'un écran soit prêt pour une nouvelle image (ou un réveil).
        let mut handles = vec![wake];
        let mut owners = Vec::new();
        let mut blocking = Vec::new();
        for &i in &live {
            match surfaces[i].waitable {
                Some(h) => {
                    handles.push(h);
                    owners.push(i);
                }
                None => blocking.push(i),
            }
        }
        let timeout = if blocking.is_empty() { 1000 } else { 0 };
        let r = unsafe { WaitForMultipleObjects(&handles, false, timeout) }.0;
        let to_render: Vec<usize> = if r == WAIT_OBJECT_0.0 {
            continue;
        } else if r > WAIT_OBJECT_0.0 && ((r - WAIT_OBJECT_0.0) as usize) < handles.len() {
            vec![owners[(r - WAIT_OBJECT_0.0) as usize - 1]]
        } else if r == WAIT_TIMEOUT.0 {
            blocking
        } else {
            unsafe { WaitForSingleObject(wake, 100) };
            continue;
        };

        // 4. Dessiner.
        let now = Instant::now();
        let dt = last_tick.map(|t| (now - t).as_secs_f64().min(0.1)).unwrap_or(1.0 / settings.fps.max(1) as f64);
        last_tick = Some(now);
        anim_time += dt * settings.speed as f64;
        if settings.mouse {
            input.poll();
        }
        let son = audio.as_mut().map(|a| a.update(dt as f32)).unwrap_or([0.0; 4]);
        cost[0] += now.elapsed().as_secs_f64();
        let heure = local_hour();
        let mut lost = false;
        for i in to_render {
            let s = &mut surfaces[i];
            let mut k = Constants {
                resolution: [s.w as f32, s.h as f32, 1.0],
                time: (anim_time % 86400.0) as f32,
                son,
                heure,
                ecran: s.index as f32,
                dt: dt as f32,
                frame: s.frames as f32,
                mouse: [-10000.0, -10000.0, -10000.0, -10000.0],
                souris: [-1.0, -1.0],
                clics: [[-10000.0, -10000.0, 99.0, 0.0]; 4],
                ..Default::default()
            };
            let on_screen = settings.mouse && inside(&s.desc.rect, input.pos);
            let moving = on_screen && input.last_move.elapsed().as_secs_f32() < 4.0;
            let target = if moving { 1.0 } else { 0.0 };
            let rate = if moving { 3.0 } else { 0.8 };
            s.presence += (target - s.presence) * (1.0 - (-rate * dt as f32).exp());
            k.presence = s.presence;
            if settings.mouse && (on_screen || s.presence > 0.001) {
                let l = to_local(s, input.pos);
                k.mouse[0] = l[0];
                k.mouse[1] = l[1];
                let (rw, rh) = rect_size(&s.desc.rect);
                if on_screen {
                    k.souris = [
                        (input.pos.x - s.desc.rect.left) as f32 / rw as f32,
                        (s.desc.rect.bottom - input.pos.y) as f32 / rh as f32,
                    ];
                }
            }
            if settings.mouse {
                let mut newest: Option<(Instant, [f32; 2])> = None;
                for (j, (p, t)) in input.clicks.iter().enumerate() {
                    if let Some(t) = t {
                        if inside(&s.desc.rect, *p) {
                            let l = to_local(s, *p);
                            k.clics[j] = [l[0], l[1], t.elapsed().as_secs_f32() * settings.speed, 0.0];
                            if newest.is_none_or(|(n, _)| *t > n) {
                                newest = Some((*t, l));
                            }
                        }
                    }
                }
                if let Some((_, l)) = newest {
                    k.mouse[2] = l[0];
                    k.mouse[3] = l[1];
                }
            }
            let t0 = Instant::now();
            if let Err(e) = g.draw(&s.rtv, s.w, s.h, &k) {
                crate::log!("dessin : {e}");
            }
            let t1 = Instant::now();
            let hr = unsafe { s.swap.Present(s.interval, DXGI_PRESENT(0)) };
            cost[1] += (t1 - t0).as_secs_f64();
            cost[2] += t1.elapsed().as_secs_f64();
            s.frames = s.frames.wrapping_add(1);
            stats.1 += 1;
            if DEVICE_LOST.contains(&hr) {
                lost = true;
                break;
            }
        }
        if stats.0.elapsed().as_secs() >= 60 {
            let per = |c: f64| c * 1e6 / stats.1.max(1) as f64;
            crate::log!(
                "{:.1} images/s sur {} écran(s) ; par image : préparation {:.0} µs, dessin {:.0} µs, présentation {:.0} µs",
                stats.1 as f32 / 60.0,
                live.len(),
                per(cost[0]),
                per(cost[1]),
                per(cost[2])
            );
            stats = (Instant::now(), 0);
            cost = [0.0; 3];
        }
        if lost {
            crate::log!("carte graphique perdue (pilote mis à jour ?) : réinitialisation");
            surfaces.clear();
            gpu = None;
            seen_surfaces = u64::MAX;
            seen_shader = u64::MAX;
        }
    }
    drop(audio);
    surfaces.clear();
    drop(gpu);
    post(ui, WM_RENDER_DONE, 0, None);
}

/// Rendu hors écran d'une image (tests et aperçus) : écrit un BMP.
pub fn capture(source: &str, file: &str, w: u32, h: u32, time: f32, out: &std::path::Path) -> Result<(), String> {
    let mut gpu = Gpu::new()?;
    gpu.set_shader(file, source)?;
    unsafe {
        let td = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            ..Default::default()
        };
        let mut tex = None;
        gpu.device.CreateTexture2D(&td, None, Some(&mut tex)).map_err(|e| e.to_string())?;
        let tex = tex.unwrap();
        let mut rtv = None;
        gpu.device.CreateRenderTargetView(&tex, None, Some(&mut rtv)).map_err(|e| e.to_string())?;
        let (fw, fh) = (w as f32, h as f32);
        let k = Constants {
            resolution: [fw, fh, 1.0],
            time,
            mouse: [fw * 0.62, fh * 0.55, fw * 0.3, fh * 0.4],
            son: [0.5, 0.3, 0.3, 0.4],
            clics: [
                [fw * 0.3, fh * 0.4, 1.2, 0.0],
                [0.0, 0.0, 99.0, 0.0],
                [0.0, 0.0, 99.0, 0.0],
                [0.0, 0.0, 99.0, 0.0],
            ],
            souris: [0.62, 0.55],
            presence: 1.0,
            heure: 21.5,
            dt: 1.0 / 30.0,
            ..Default::default()
        };
        gpu.draw(rtv.as_ref().unwrap(), w, h, &k)?;
        let sd = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..td
        };
        let mut staging = None;
        gpu.device.CreateTexture2D(&sd, None, Some(&mut staging)).map_err(|e| e.to_string())?;
        let staging = staging.unwrap();
        gpu.ctx.CopyResource(&staging, &tex);
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        gpu.ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m)).map_err(|e| e.to_string())?;
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        // BMP : lignes de bas en haut.
        for y in (0..h).rev() {
            let row =
                std::slice::from_raw_parts((m.pData as *const u8).add((y * m.RowPitch) as usize), (w * 4) as usize);
            pixels.extend_from_slice(row);
        }
        gpu.ctx.Unmap(&staging, 0);
        write_bmp(out, w, h, &pixels)
    }
}

fn write_bmp(path: &std::path::Path, w: u32, h: u32, bgra: &[u8]) -> Result<(), String> {
    let size = 54 + bgra.len() as u32;
    let mut b = Vec::with_capacity(size as usize);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(w as i32).to_le_bytes());
    b.extend_from_slice(&(h as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(&[0u8; 24]);
    b.extend_from_slice(bgra);
    std::fs::write(path, b).map_err(|e| e.to_string())
}

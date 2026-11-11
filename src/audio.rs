//! Réactivité au son : capture « loopback » WASAPI de la sortie par défaut
//! (ce que l'on entend), lue par scrutation à chaque image — aucun fil
//! supplémentaire. Une FFT de 1024 points donne graves, médiums et aigus.
//! N'est ouverte que si le fond affiché utilise `uSon`.

use std::time::Instant;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};

const N: usize = 1024;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

pub struct Audio {
    enumerator: IMMDeviceEnumerator,
    device_id: String,
    client: IAudioClient,
    capture: IAudioCaptureClient,
    channels: usize,
    float: bool,
    bits: u16,
    rate: f32,
    /// Sorties à 96/192 kHz : on moyenne par paquets pour revenir vers 48 kHz,
    /// sinon la FFT n'aurait presque aucune résolution dans les graves.
    decim: usize,
    acc: (f32, usize),
    ring: Vec<f32>,
    pos: usize,
    window: Vec<f32>,
    fft: Fft,
    bands: [f32; 4],
    peak: [f32; 4],
    checked: Instant,
}

fn default_device(e: &IMMDeviceEnumerator) -> windows::core::Result<(IMMDevice, String)> {
    unsafe {
        let d = e.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let p = d.GetId()?;
        let id = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        Ok((d, id))
    }
}

impl Audio {
    pub fn new() -> Result<Audio, String> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            let (device, device_id) = default_device(&enumerator).map_err(|e| format!("pas de sortie audio : {e}"))?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
            let fmt = client.GetMixFormat().map_err(|e| e.to_string())?;
            let f = *fmt;
            let tag = f.wFormatTag;
            let float = if tag == WAVE_FORMAT_EXTENSIBLE {
                let ext = &*(fmt as *const WAVEFORMATEXTENSIBLE);
                let sub = ext.SubFormat;
                // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT = 00000003-0000-0010-8000-00aa00389b71
                sub.data1 == 3
            } else {
                tag == WAVE_FORMAT_IEEE_FLOAT
            };
            let r = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 2_000_000, 0, fmt, None);
            let (channels, bits, rate) = (f.nChannels as usize, f.wBitsPerSample, f.nSamplesPerSec as f32);
            CoTaskMemFree(Some(fmt as *const _));
            r.map_err(|e| e.to_string())?;
            let capture: IAudioCaptureClient = client.GetService().map_err(|e| e.to_string())?;
            client.Start().map_err(|e| e.to_string())?;
            let window = (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()).collect();
            crate::log!(
                "capture du son : {rate} Hz, {channels} canaux, {bits} bits{}",
                if float { " flottants" } else { "" }
            );
            Ok(Audio {
                enumerator,
                device_id,
                client,
                capture,
                channels: channels.max(1),
                float,
                bits,
                rate: rate / (rate / 48000.0).round().max(1.0),
                decim: (rate / 48000.0).round().max(1.0) as usize,
                acc: (0.0, 0),
                ring: vec![0.0; N],
                pos: 0,
                window,
                fft: Fft::new(N),
                bands: [0.0; 4],
                peak: [0.05; 4],
                checked: Instant::now(),
            })
        }
    }

    fn drain(&mut self) -> windows::core::Result<()> {
        unsafe {
            loop {
                let n = self.capture.GetNextPacketSize()?;
                if n == 0 {
                    return Ok(());
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                let silent = flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
                let ch = self.channels;
                for i in 0..frames as usize {
                    let mut v = 0.0f32;
                    if !silent {
                        for c in 0..ch.min(2) {
                            let idx = i * ch + c;
                            v += if self.float && self.bits == 32 {
                                *(data as *const f32).add(idx)
                            } else if self.bits == 16 {
                                *(data as *const i16).add(idx) as f32 / 32768.0
                            } else if self.bits == 32 {
                                *(data as *const i32).add(idx) as f32 / 2147483648.0
                            } else {
                                0.0
                            };
                        }
                        v /= ch.min(2) as f32;
                    }
                    self.acc.0 += v;
                    self.acc.1 += 1;
                    if self.acc.1 >= self.decim {
                        self.ring[self.pos] = self.acc.0 / self.acc.1 as f32;
                        self.pos = (self.pos + 1) % N;
                        self.acc = (0.0, 0);
                    }
                }
                self.capture.ReleaseBuffer(frames)?;
            }
        }
    }

    /// Lit le son arrivé depuis la dernière image et renvoie
    /// [graves, médiums, aigus, volume] normalisés et lissés.
    pub fn update(&mut self, dt: f32) -> [f32; 4] {
        if self.checked.elapsed().as_secs() >= 3 {
            self.checked = Instant::now();
            // Changement de sortie par défaut (casque branché…) : on suit.
            if let Ok((_, id)) = default_device(&self.enumerator) {
                if id != self.device_id {
                    if let Ok(a) = Audio::new() {
                        *self = a;
                    }
                }
            }
        }
        if let Err(e) = self.drain() {
            if e.code() == AUDCLNT_E_DEVICE_INVALIDATED {
                if let Ok(a) = Audio::new() {
                    *self = a;
                }
            }
            return self.decay(dt);
        }

        let mut re = [0.0f32; N];
        let mut im = [0.0f32; N];
        let mut sq = 0.0;
        for (i, (r, w)) in re.iter_mut().zip(&self.window).enumerate() {
            let s = self.ring[(self.pos + i) % N];
            sq += s * s;
            *r = s * w;
        }
        self.fft.run(&mut re, &mut im);
        let bin_hz = self.rate / N as f32;
        let band = |lo: f32, hi: f32| {
            let a = ((lo / bin_hz) as usize).max(1);
            let b = ((hi / bin_hz) as usize).clamp(a + 1, N / 2);
            let sum: f32 = (a..b).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).sum();
            sum / (b - a) as f32
        };
        let raw = [band(25.0, 160.0), band(160.0, 2000.0), band(2000.0, 10000.0), (sq / N as f32).sqrt()];
        let mut out = [0.0; 4];
        for i in 0..4 {
            // Gain automatique : chaque bande est rapportée à son maximum récent.
            self.peak[i] = (self.peak[i] * (-dt / 6.0).exp()).max(raw[i]).max(1e-4);
            let v = (raw[i] / self.peak[i]).clamp(0.0, 1.0);
            let v = if raw[i] < 1e-4 { 0.0 } else { v * v };
            let k = if v > self.bands[i] { 1.0 - (-dt * 25.0).exp() } else { 1.0 - (-dt * 4.0).exp() };
            self.bands[i] += (v - self.bands[i]) * k;
            out[i] = self.bands[i];
        }
        out
    }

    fn decay(&mut self, dt: f32) -> [f32; 4] {
        for b in &mut self.bands {
            *b *= (-dt * 4.0).exp();
        }
        self.bands
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

/// FFT radix-2 en place, tables précalculées.
struct Fft {
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<usize>,
}

impl Fft {
    fn new(n: usize) -> Fft {
        let bits = n.trailing_zeros();
        let rev = (0..n).map(|i| i.reverse_bits() >> (usize::BITS - bits)).collect();
        let (cos, sin) = (0..n / 2)
            .map(|k| {
                let a = -2.0 * std::f32::consts::PI * k as f32 / n as f32;
                (a.cos(), a.sin())
            })
            .unzip();
        Fft { cos, sin, rev }
    }

    fn run(&self, re: &mut [f32], im: &mut [f32]) {
        let n = re.len();
        for i in 0..n {
            let j = self.rev[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let step = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..len / 2 {
                    let (c, s) = (self.cos[k * step], self.sin[k * step]);
                    let (a, b) = (start + k, start + k + len / 2);
                    let tr = re[b] * c - im[b] * s;
                    let ti = re[b] * s + im[b] * c;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_sinus() {
        let fft = Fft::new(N);
        let mut re: Vec<f32> = (0..N).map(|i| (2.0 * std::f32::consts::PI * 8.0 * i as f32 / N as f32).sin()).collect();
        let mut im = vec![0.0; N];
        fft.run(&mut re, &mut im);
        let mag: Vec<f32> = (0..N / 2).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).collect();
        let best = mag.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
        assert_eq!(best, 8);
    }
}

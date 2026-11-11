//! Réglages, stockés dans `%APPDATA%\Onde\onde.ini` (lignes `clé = valeur`).

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Identifiant du fond : nom intégré, ou `perso:fichier.hlsl`.
    pub fond: String,
    pub fps: u32,
    /// Résolution de rendu en % de celle de l'écran (l'image est étirée).
    pub echelle: u32,
    /// Vitesse d'animation en %.
    pub vitesse: u32,
    pub son: bool,
    pub souris: bool,
    pub pause_batterie: bool,
    pub pause_hors_bureau: bool,
    pub pause: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            fond: "aurore".into(),
            fps: 30,
            echelle: 50,
            vitesse: 100,
            son: true,
            souris: true,
            pause_batterie: true,
            pause_hors_bureau: false,
            pause: false,
        }
    }
}

pub const FPS_CHOICES: [u32; 5] = [10, 15, 24, 30, 60];
pub const SCALE_CHOICES: [u32; 5] = [100, 75, 50, 33, 25];
pub const SPEED_CHOICES: [u32; 5] = [25, 50, 100, 150, 200];

fn parse_bool(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "oui" | "true" | "1" | "on" => Some(true),
        "non" | "false" | "0" | "off" => Some(false),
        _ => None,
    }
}

fn yes(b: bool) -> &'static str {
    if b {
        "oui"
    } else {
        "non"
    }
}

impl Config {
    pub fn path() -> PathBuf {
        crate::util::data_dir().join("onde.ini")
    }

    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "fond" if !v.is_empty() => c.fond = v.to_string(),
                "fps" => c.fps = v.parse().unwrap_or(c.fps).clamp(1, 240),
                "echelle" => c.echelle = v.parse().unwrap_or(c.echelle).clamp(10, 100),
                "vitesse" => c.vitesse = v.parse().unwrap_or(c.vitesse).clamp(5, 1000),
                "son" => c.son = parse_bool(v).unwrap_or(c.son),
                "souris" => c.souris = parse_bool(v).unwrap_or(c.souris),
                "pause_batterie" => c.pause_batterie = parse_bool(v).unwrap_or(c.pause_batterie),
                "pause_hors_bureau" => c.pause_hors_bureau = parse_bool(v).unwrap_or(c.pause_hors_bureau),
                "pause" => c.pause = parse_bool(v).unwrap_or(c.pause),
                _ => {}
            }
        }
        c
    }

    pub fn load() -> Config {
        std::fs::read_to_string(Self::path()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let text = format!(
            "# Réglages d'Onde (modifiables aussi depuis l'icône de notification)\n\
             fond = {}\nfps = {}\nechelle = {}\nvitesse = {}\nson = {}\nsouris = {}\n\
             pause_batterie = {}\npause_hors_bureau = {}\npause = {}\n",
            self.fond,
            self.fps,
            self.echelle,
            self.vitesse,
            yes(self.son),
            yes(self.souris),
            yes(self.pause_batterie),
            yes(self.pause_hors_bureau),
            yes(self.pause)
        );
        let _ = std::fs::create_dir_all(crate::util::data_dir());
        std::fs::write(Self::path(), text).map_err(|e| format!("impossible d'enregistrer les réglages : {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        let c = Config { fond: "perso:x.hlsl".into(), fps: 24, son: false, ..Config::default() };
        let text = format!("fond = {}\nfps = {}\nson = {}\n", c.fond, c.fps, if c.son { "oui" } else { "non" });
        assert_eq!(Config::parse(&text), c);
    }

    #[test]
    fn bornes() {
        let c = Config::parse("fps = 9999\nechelle = 1\n");
        assert_eq!(c.fps, 240);
        assert_eq!(c.echelle, 10);
    }
}

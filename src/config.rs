//! Réglages, stockés dans `%APPDATA%\Onde\onde.ini` (lignes `clé = valeur`).

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Identifiant du fond : nom intégré, ou `perso:fichier.hlsl`.
    pub fond: String,
    /// Images par seconde ; [`FPS_ECRAN`] = fréquence de l'écran,
    /// [`FPS_ILLIMITE`] = sans limite.
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
    /// Un seul fond continu sur tout le bureau virtuel (sinon un par écran).
    pub continu: bool,
    /// Recopier une image du fond sur l'écran de verrouillage.
    pub verrouillage: bool,
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
            continu: true,
            verrouillage: false,
        }
    }
}

pub const FPS_ECRAN: u32 = 0;
pub const FPS_ILLIMITE: u32 = u32::MAX;
pub const FPS_CHOICES: [u32; 9] = [5, 10, 15, 24, 30, 60, 120, FPS_ECRAN, FPS_ILLIMITE];
pub const SCALE_CHOICES: [u32; 8] = [200, 150, 100, 75, 50, 33, 25, 10];
pub const SPEED_CHOICES: [u32; 9] = [10, 25, 50, 100, 150, 200, 300, 500, 1000];

/// Bornes de sécurité seulement (valeurs absurdes dans le fichier) :
/// tout ce qui est raisonnable passe, y compris hors du menu.
pub const SCALE_MAX: u32 = 400;
pub const SPEED_MAX: u32 = 100_000;

pub fn fps_label(fps: u32) -> String {
    match fps {
        FPS_ECRAN => "fréquence de l'écran".into(),
        FPS_ILLIMITE => "illimité".into(),
        n => format!("{n} images/s"),
    }
}

fn parse_fps(v: &str) -> Option<u32> {
    match v.to_ascii_lowercase().as_str() {
        "ecran" | "écran" | "max" | "0" => Some(FPS_ECRAN),
        "illimite" | "illimité" | "illimitee" | "illimitée" => Some(FPS_ILLIMITE),
        n => n.parse::<u32>().ok().filter(|n| *n >= 1),
    }
}

fn fps_text(fps: u32) -> String {
    match fps {
        FPS_ECRAN => "ecran".into(),
        FPS_ILLIMITE => "illimite".into(),
        n => n.to_string(),
    }
}

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
        // Le Bloc-notes enregistre parfois avec une marque d'ordre des octets (BOM).
        for line in text.trim_start_matches('\u{feff}').lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "fond" if !v.is_empty() => c.fond = v.to_string(),
                "fps" => c.fps = parse_fps(v).unwrap_or(c.fps),
                "echelle" => c.echelle = v.parse().unwrap_or(c.echelle).clamp(1, SCALE_MAX),
                "vitesse" => c.vitesse = v.parse().unwrap_or(c.vitesse).clamp(1, SPEED_MAX),
                "son" => c.son = parse_bool(v).unwrap_or(c.son),
                "souris" => c.souris = parse_bool(v).unwrap_or(c.souris),
                "pause_batterie" => c.pause_batterie = parse_bool(v).unwrap_or(c.pause_batterie),
                "pause_hors_bureau" => c.pause_hors_bureau = parse_bool(v).unwrap_or(c.pause_hors_bureau),
                "pause" => c.pause = parse_bool(v).unwrap_or(c.pause),
                "ecrans" => match v.to_ascii_lowercase().as_str() {
                    "continu" => c.continu = true,
                    "separes" | "séparés" | "separe" | "séparé" => c.continu = false,
                    _ => {}
                },
                "verrouillage" => c.verrouillage = parse_bool(v).unwrap_or(c.verrouillage),
                _ => {}
            }
        }
        c
    }

    pub fn load() -> Config {
        std::fs::read_to_string(Self::path()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn to_text(&self) -> String {
        format!(
            "# Réglages d'Onde (modifiables aussi depuis l'icône de notification)
             # fps : un nombre, « ecran » (fréquence de l'écran) ou « illimite »
             fond = {}
fps = {}
echelle = {}
vitesse = {}
son = {}
souris = {}
             pause_batterie = {}
pause_hors_bureau = {}
pause = {}
             # ecrans : continu (un seul fond sur tout le bureau) ou separes
             ecrans = {}
verrouillage = {}
",
            self.fond,
            fps_text(self.fps),
            self.echelle,
            self.vitesse,
            yes(self.son),
            yes(self.souris),
            yes(self.pause_batterie),
            yes(self.pause_hors_bureau),
            yes(self.pause),
            if self.continu { "continu" } else { "separes" },
            yes(self.verrouillage)
        )
    }

    pub fn save(&self) -> Result<(), String> {
        let _ = std::fs::create_dir_all(crate::util::data_dir());
        std::fs::write(Self::path(), self.to_text()).map_err(|e| format!("impossible d'enregistrer les réglages : {e}"))
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
        let c = Config::parse("fps = 9999\nechelle = 1000\nvitesse = 0\n");
        assert_eq!(c.fps, 9999);
        assert_eq!(c.echelle, SCALE_MAX);
        assert_eq!(c.vitesse, 1);
        assert_eq!(Config::parse("fps = ecran").fps, FPS_ECRAN);
        assert_eq!(Config::parse("fps = illimite").fps, FPS_ILLIMITE);
        assert_eq!(Config::parse("fps = 0").fps, FPS_ECRAN);
    }

    #[test]
    fn marque_bom() {
        assert_eq!(Config::parse("\u{feff}fond = horizon\nfps = 24\n").fond, "horizon");
    }

    #[test]
    fn enregistrement_complet() {
        let c = Config { fps: FPS_ILLIMITE, continu: false, verrouillage: true, echelle: 150, ..Config::default() };
        assert_eq!(Config::parse(&c.to_text()), c);
    }
}

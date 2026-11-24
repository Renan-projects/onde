//! Disposition des écrans dans un espace commun, pour qu'un fond soit
//! continu d'un écran à l'autre.
//!
//! Windows place les écrans en pixels physiques. Quand leurs échelles (PPP)
//! diffèrent, un motif de même taille en pixels paraît plus petit sur
//! l'écran le plus dense : on travaille donc en pixels « logiques »
//! (physiques ÷ facteur d'échelle). Chaque écran garde sa taille logique et
//! est accolé à un voisin déjà placé le long du bord qu'ils partagent, ce
//! qui garde la jonction continue même quand les échelles diffèrent.

/// Rectangle en pixels physiques (gauche, haut, droite, bas).
pub type PRect = (i32, i32, i32, i32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// Coin haut-gauche dans l'espace logique commun (y vers le bas).
    pub x: f32,
    pub y: f32,
    /// Pixels physiques par pixel logique (1,0 à 100 %, 1,5 à 150 %…).
    pub f: f32,
}

/// Place chaque écran (`rect`, `dpi`) dans l'espace logique commun.
/// L'écran de référence est celui qui contient l'origine (écran principal),
/// à défaut le premier.
pub fn arrange(mons: &[(PRect, u32)]) -> Vec<Placed> {
    let n = mons.len();
    let fac = |i: usize| (mons[i].1.max(1) as f32 / 96.0).max(0.1);
    let mut out: Vec<Option<Placed>> = vec![None; n];
    if n == 0 {
        return Vec::new();
    }
    let root = mons.iter().position(|(r, _)| r.0 <= 0 && r.1 <= 0 && r.2 > 0 && r.3 > 0).unwrap_or(0);
    let fr = fac(root);
    out[root] = Some(Placed { x: mons[root].0 .0 as f32 / fr, y: mons[root].0 .1 as f32 / fr, f: fr });
    // Propagation de proche en proche par les bords partagés.
    loop {
        let mut progressed = false;
        for i in 0..n {
            if out[i].is_some() {
                continue;
            }
            let (a, fa) = (mons[i].0, fac(i));
            for j in 0..n {
                let Some(pj) = out[j] else { continue };
                let b = mons[j].0;
                let fb = pj.f;
                let overlap_y = a.1 < b.3 && b.1 < a.3;
                let overlap_x = a.0 < b.2 && b.0 < a.2;
                let place = if overlap_y && a.0 == b.2 {
                    // i à droite de j
                    Some((pj.x + (b.2 - b.0) as f32 / fb, pj.y + (a.1 - b.1) as f32 / fb))
                } else if overlap_y && a.2 == b.0 {
                    // i à gauche de j
                    Some((pj.x - (a.2 - a.0) as f32 / fa, pj.y + (a.1 - b.1) as f32 / fb))
                } else if overlap_x && a.1 == b.3 {
                    // i sous j
                    Some((pj.x + (a.0 - b.0) as f32 / fb, pj.y + (b.3 - b.1) as f32 / fb))
                } else if overlap_x && a.3 == b.1 {
                    // i au-dessus de j
                    Some((pj.x + (a.0 - b.0) as f32 / fb, pj.y - (a.3 - a.1) as f32 / fa))
                } else {
                    None
                };
                if let Some((x, y)) = place {
                    out[i] = Some(Placed { x, y, f: fa });
                    progressed = true;
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    // Écrans isolés (ne touchant aucun autre) : position physique ramenée à l'échelle de référence.
    (0..n)
        .map(|i| out[i].unwrap_or(Placed { x: mons[i].0 .0 as f32 / fr, y: mons[i].0 .1 as f32 / fr, f: fac(i) }))
        .collect()
}

/// Vue d'un écran dans l'espace du fond (unités = pixels de rendu).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    /// `fragCoord = (ox + px * k, oy - py * k)` pour le pixel de rendu (px, py),
    /// origine en haut à gauche de la surface.
    pub ox: f32,
    pub oy: f32,
    pub k: f32,
}

/// Monde commun : taille totale (`iResolution`) et vue de chaque écran.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct World {
    pub size: [f32; 2],
    pub views: Vec<View>,
    /// Pour convertir un point écran : rectangle physique, placement, échelle de rendu.
    mons: Vec<(PRect, Placed)>,
    left: f32,
    bottom: f32,
    scale: f32,
}

impl World {
    /// `surfaces` : (rectangle physique, ppp, taille de rendu w×h) de chaque écran.
    pub fn continuous(surfaces: &[(PRect, u32, u32, u32)], scale: f32) -> World {
        let placed = arrange(&surfaces.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>());
        let mut l = f32::MAX;
        let mut t = f32::MAX;
        let mut r = f32::MIN;
        let mut b = f32::MIN;
        for (s, p) in surfaces.iter().zip(&placed) {
            l = l.min(p.x);
            t = t.min(p.y);
            r = r.max(p.x + (s.0 .2 - s.0 .0) as f32 / p.f);
            b = b.max(p.y + (s.0 .3 - s.0 .1) as f32 / p.f);
        }
        if surfaces.is_empty() {
            (l, t, r, b) = (0.0, 0.0, 1.0, 1.0);
        }
        let views = surfaces
            .iter()
            .zip(&placed)
            .map(|(s, p)| {
                let rw = (s.0 .2 - s.0 .0).max(1) as f32;
                View { ox: (p.x - l) * scale, oy: (b - p.y) * scale, k: scale * rw / (s.2.max(1) as f32 * p.f) }
            })
            .collect();
        World {
            size: [((r - l) * scale).max(1.0), ((b - t) * scale).max(1.0)],
            views,
            mons: surfaces.iter().zip(&placed).map(|(s, p)| (s.0, *p)).collect(),
            left: l,
            bottom: b,
            scale,
        }
    }

    /// Point écran (pixels physiques) → coordonnées du fond, ou `None` hors de tout écran.
    pub fn point(&self, x: i32, y: i32) -> Option<[f32; 2]> {
        let m = self.mons.iter().find(|(r, _)| x >= r.0 && x < r.2 && y >= r.1 && y < r.3)?;
        Some(self.map(m, x, y))
    }

    /// Comme [`World::point`], mais prolonge le repère du premier écran
    /// au-delà de ses bords (la souris s'éloigne en douceur).
    pub fn point_beyond(&self, x: i32, y: i32) -> Option<[f32; 2]> {
        self.point(x, y).or_else(|| self.mons.first().map(|m| self.map(m, x, y)))
    }

    fn map(&self, (rect, p): &(PRect, Placed), x: i32, y: i32) -> [f32; 2] {
        let lx = p.x + (x - rect.0) as f32 / p.f;
        let ly = p.y + (y - rect.1) as f32 / p.f;
        [(lx - self.left) * self.scale, (self.bottom - ly) * self.scale]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_ecrans_1080p() {
        let s = [((0, 0, 1920, 1080), 96, 960, 540), ((1920, 0, 3840, 1080), 96, 960, 540)];
        let w = World::continuous(&s, 0.5);
        assert_eq!(w.size, [1920.0, 540.0]);
        assert_eq!(w.views[0], View { ox: 0.0, oy: 540.0, k: 1.0 });
        assert_eq!(w.views[1], View { ox: 960.0, oy: 540.0, k: 1.0 });
        // Le dernier pixel de l'écran 0 et le premier de l'écran 1 se suivent.
        let v0 = w.views[0];
        let v1 = w.views[1];
        assert_eq!(v0.ox + 959.5 * v0.k + 1.0, v1.ox + 0.5 * v1.k);
        assert_eq!(w.point(1920, 0), Some([960.0, 540.0]));
        assert_eq!(w.point(5000, 0), None);
    }

    #[test]
    fn ecran_a_gauche_et_decale() {
        // Écran secondaire à gauche, plus bas de 200 px.
        let p = arrange(&[((0, 0, 1920, 1080), 96), ((-1280, 200, 0, 1224), 96)]);
        assert_eq!(p[1], Placed { x: -1280.0, y: 200.0, f: 1.0 });
    }

    #[test]
    fn echelles_differentes() {
        // 4K à 200 % à droite d'un 1080p à 100 % : 1920 pixels logiques de large.
        let p = arrange(&[((0, 0, 1920, 1080), 96), ((1920, 0, 5760, 2160), 192)]);
        assert_eq!(p[1], Placed { x: 1920.0, y: 0.0, f: 2.0 });
        let w =
            World::continuous(&[((0, 0, 1920, 1080), 96, 1920, 1080), ((1920, 0, 5760, 2160), 192, 3840, 2160)], 1.0);
        assert_eq!(w.size, [3840.0, 1080.0]);
        assert_eq!(w.views[1].k, 0.5);
        assert_eq!(w.point(5759, 2159).map(|p| p[0]), Some(3839.5));
    }
}

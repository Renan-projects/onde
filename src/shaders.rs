//! Catalogue des fonds : ceux intégrés à l'exécutable et ceux de l'utilisateur
//! (`%APPDATA%\Onde\fonds\*.hlsl`), plus l'enveloppe HLSL commune.

use std::path::PathBuf;

pub struct Builtin {
    pub id: &'static str,
    pub source: &'static str,
}

pub const BUILTINS: &[Builtin] = &[
    Builtin { id: "aurore", source: include_str!("../fonds/aurore.hlsl") },
    Builtin { id: "encre", source: include_str!("../fonds/encre.hlsl") },
    Builtin { id: "ondes", source: include_str!("../fonds/ondes.hlsl") },
    Builtin { id: "horizon", source: include_str!("../fonds/horizon.hlsl") },
    Builtin { id: "constellation", source: include_str!("../fonds/constellation.hlsl") },
    Builtin { id: "lave", source: include_str!("../fonds/lave.hlsl") },
    Builtin { id: "brume", source: include_str!("../fonds/brume.hlsl") },
];

pub const EXAMPLE: &str = include_str!("../fonds/exemple.hlsl");

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub path: Option<PathBuf>,
}

/// Nom affiché : ligne `// @nom: …` sinon l'identifiant.
pub fn display_name(source: &str, fallback: &str) -> String {
    source
        .lines()
        .take(20)
        .find_map(|l| l.trim().strip_prefix("// @nom:").map(|n| n.trim().to_string()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

/// Le fond lit-il le son ? (sinon la capture audio n'est jamais ouverte)
pub fn uses_audio(source: &str) -> bool {
    source.contains("uSon")
}

pub fn list() -> Vec<Entry> {
    let mut out: Vec<Entry> = BUILTINS
        .iter()
        .map(|b| Entry { id: b.id.to_string(), name: display_name(b.source, b.id), path: None })
        .collect();
    if let Ok(dir) = std::fs::read_dir(crate::util::user_shader_dir()) {
        let mut user: Vec<Entry> = dir
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("hlsl")))
            .filter_map(|p| {
                let file = p.file_name()?.to_string_lossy().into_owned();
                let src = std::fs::read_to_string(&p).unwrap_or_default();
                let stem = p.file_stem()?.to_string_lossy().into_owned();
                Some(Entry { id: format!("perso:{file}"), name: display_name(&src, &stem), path: Some(p) })
            })
            .collect();
        user.sort_by_key(|e| e.name.to_lowercase());
        out.extend(user);
    }
    out
}

/// Source d'un fond, ou `None` s'il n'existe plus.
pub fn source(id: &str) -> Option<String> {
    if let Some(file) = id.strip_prefix("perso:") {
        return std::fs::read_to_string(crate::util::user_shader_dir().join(file)).ok();
    }
    BUILTINS.iter().find(|b| b.id == id).map(|b| b.source.to_string())
}

/// Crée le dossier personnel avec un exemple commenté au premier lancement.
pub fn ensure_user_dir() {
    let dir = crate::util::user_shader_dir();
    if !dir.exists() && std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(dir.join("exemple.hlsl"), EXAMPLE);
    }
}

pub const VERTEX: &str = r#"
float4 VS(uint id : SV_VertexID) : SV_Position {
    float2 p = float2((id << 1) & 2, id & 2);
    return float4(p * float2(2, -2) + float2(-1, 1), 0, 1);
}
"#;

/// En-tête commun : constantes envoyées à chaque image + quelques
/// équivalences GLSL pour porter facilement un shader Shadertoy.
pub const PRELUDE: &str = r#"
cbuffer Onde : register(b0) {
    float3 iResolution;   // taille de rendu en pixels
    float  iTime;         // secondes (vitesse réglable)
    float4 iMouse;        // xy : souris en pixels (origine en bas à gauche), zw : dernier clic
    float4 uSon;          // x graves, y médiums, z aigus, w volume (0..1, lissés)
    float4 uClics[4];     // xy : position en pixels, z : âge en secondes (99 = aucun)
    float2 uSouris;       // souris en 0..1 (-1 si elle est sur un autre écran)
    float  uPresence;     // 0..1 : la souris est sur cet écran et bouge
    float  uHeure;        // heure locale, 0..24
    float  uEcran;        // numéro de l'écran
    float  iTimeDelta;
    float  iFrame;
    float  uPad;
};
#define vec2 float2
#define vec3 float3
#define vec4 float4
#define mat2 float2x2
#define mat3 float3x3
#define fract frac
#define mix lerp
float  mod(float  x, float  y) { return x - y * floor(x / y); }
float2 mod(float2 x, float2 y) { return x - y * floor(x / y); }
float3 mod(float3 x, float3 y) { return x - y * floor(x / y); }
float2 mod(float2 x, float  y) { return x - y * floor(x / y); }
float3 mod(float3 x, float  y) { return x - y * floor(x / y); }
"#;

pub const EPILOGUE: &str = r#"
float4 PS(float4 pos : SV_Position) : SV_Target {
    float2 fc = float2(pos.x, iResolution.y - pos.y);
    float3 c = saturate(mainImage(fc).rgb);
    // Tramage léger : évite les bandes dans les dégradés en 8 bits.
    float n = frac(sin(dot(pos.xy + frac(iTime) * 61.0, float2(12.9898, 78.233))) * 43758.5453);
    return float4(c + (n - 0.5) / 255.0, 1);
}
"#;

pub fn pixel_source(user: &str, file: &str) -> String {
    format!("{PRELUDE}\n#line 1 \"{file}\"\n{user}\n#line 1 \"onde\"\n{EPILOGUE}")
}

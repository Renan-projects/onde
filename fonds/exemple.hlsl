// @nom: Exemple (à modifier)
//
// Fond d'écran personnel pour Onde. Tout fichier .hlsl de ce dossier
// apparaît dans le menu « Fond d'écran » ; il est recompilé dès qu'il est
// enregistré, et une erreur s'affiche en notification.
//
// Écrire une fonction  float4 mainImage(float2 fragCoord)  comme sur
// Shadertoy (fragCoord en pixels, origine en bas à gauche).
// HLSL : float2/float3, lerp, frac, mul(m, v)… vec2/vec3/mix/fract/mod sont
// aussi acceptés pour faciliter le portage depuis GLSL.
//
// Variables disponibles :
//   iResolution.xy  taille de rendu      iTime       temps en secondes
//   iMouse.xy       souris en pixels     iMouse.zw   dernier clic
//   uSouris         souris en 0..1 (-1 : autre écran)
//   uPresence       0..1, la souris est là et bouge
//   uClics[4]       xy : position des 4 derniers clics, z : âge en secondes
//   uSon            x graves, y médiums, z aigus, w volume (0..1)
//                   (la capture du son n'est activée que si le fond s'en sert)
//   uHeure          heure locale (0..24)     uEcran   numéro de l'écran

float4 mainImage(float2 fragCoord) {
    float2 uv = fragCoord / iResolution.xy;
    // Couleur qui suit l'heure de la journée : nuit bleue, jour doré.
    float jour = smoothstep(6.0, 9.0, uHeure) * smoothstep(21.0, 18.0, uHeure);
    float3 haut = lerp(float3(0.02, 0.03, 0.10), float3(0.25, 0.45, 0.80), jour);
    float3 bas = lerp(float3(0.10, 0.03, 0.12), float3(0.95, 0.70, 0.45), jour);
    float3 col = lerp(bas, haut, uv.y + 0.05 * sin(uv.x * 6.0 + iTime * 0.5));
    // Halo sous la souris, qui grossit avec les basses.
    float d = length((fragCoord - iMouse.xy) / iResolution.y);
    col += uPresence * 0.25 * exp(-d * (12.0 - 6.0 * uSon.x));
    return float4(col, 1);
}

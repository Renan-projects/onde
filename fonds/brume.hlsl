// @nom: Brume (le plus léger)
// Quelques nuées de couleur qui dérivent très lentement. Presque aucun
// calcul : idéal pour un portable. La souris apporte une lueur douce.

float4 mainImage(float2 fc) {
    float2 uv = fc / iResolution.xy;
    float asp = iResolution.x / iResolution.y;
    float2 p = float2(uv.x * asp, uv.y);
    float t = iTime * 0.05 + uEcran * 4.0;

    float3 col = float3(0.03, 0.035, 0.07);
    float2 b1 = float2(asp * (0.3 + 0.2 * sin(t * 1.1)), 0.6 + 0.25 * cos(t * 0.8));
    float2 b2 = float2(asp * (0.7 + 0.2 * cos(t * 0.9)), 0.4 + 0.25 * sin(t * 1.3));
    float2 b3 = float2(asp * (0.5 + 0.35 * sin(t * 0.6 + 2.0)), 0.2 + 0.2 * sin(t * 0.7));
    float2 b4 = float2(asp * (0.5 + 0.3 * cos(t * 0.5 + 1.0)), 0.85 + 0.1 * cos(t));
    col += float3(0.30, 0.12, 0.45) * exp(-dot(p - b1, p - b1) * 3.0);
    col += float3(0.05, 0.28, 0.40) * exp(-dot(p - b2, p - b2) * 3.0);
    col += float3(0.45, 0.18, 0.20) * exp(-dot(p - b3, p - b3) * 4.0);
    col += float3(0.10, 0.30, 0.22) * exp(-dot(p - b4, p - b4) * 4.0);
    col *= 0.75 + 0.35 * uSon.w;

    float2 m = float2(uSouris.x * asp, uSouris.y);
    col += float3(0.12, 0.10, 0.15) * uPresence * exp(-dot(p - m, p - m) * 18.0);
    return float4(col, 1);
}

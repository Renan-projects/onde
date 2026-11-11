// @nom: Horizon
// Coucher de soleil rétro sur une grille qui défile. Le soleil suit la
// souris, la grille s'illumine au rythme des basses.

float hash11(float x) { return frac(sin(x * 91.7) * 43758.5453); }

float4 mainImage(float2 fc) {
    float2 uv = fc / iResolution.xy;
    float2 p = (fc - 0.5 * iResolution.xy) / iResolution.y;
    float horizon = -0.05;
    float bass = uSon.x;

    // Ciel
    float3 col = lerp(float3(0.95, 0.35, 0.35), float3(0.06, 0.02, 0.14), saturate((p.y - horizon) * 2.2));
    col = lerp(col, float3(0.02, 0.0, 0.06), saturate((p.y - 0.25) * 2.0));

    // Soleil (suit doucement la souris en horizontal)
    float sx = uSouris.x >= 0.0 ? (uSouris.x - 0.5) * 0.25 * (iResolution.x / iResolution.y) * uPresence : 0.0;
    float2 sc = float2(sx, horizon + 0.2);
    float r = 0.22 + 0.015 * bass;
    float d = length(p - sc);
    float3 sun = lerp(float3(1.0, 0.85, 0.3), float3(1.0, 0.2, 0.55), saturate((sc.y + r - p.y) / (2.0 * r)));
    float stripes = step(0.0, sin((p.y - sc.y) * 90.0 + iTime * 2.0) + (p.y - sc.y) * 18.0 + 1.2);
    if (p.y < sc.y) stripes = step(0.0, sin(p.y * 70.0 - iTime * 1.2) + (p.y - sc.y) * 25.0 + 1.5);
    float disc = smoothstep(r, r - 0.004, d) * stripes;
    col = lerp(col, sun, disc);
    col += float3(1.0, 0.3, 0.5) * 0.25 * exp(-max(d - r, 0.0) * 9.0) * (1.0 - disc);

    // Montagnes
    float mx = p.x * 3.0;
    float mount = horizon + 0.06 * abs(sin(mx * 1.3 + 1.7)) + 0.04 * abs(sin(mx * 3.1 + 0.3)) + 0.02 * abs(sin(mx * 7.3));
    if (p.y < mount && p.y > horizon) col = lerp(float3(0.06, 0.01, 0.1), col, 0.15);

    // Grille au sol
    if (p.y < horizon) {
        float depth = (horizon - p.y);
        float z = 0.35 / depth;
        float x = p.x * z;
        float zz = z + iTime * 1.2;
        float lw = 0.03 * z;
        float gx = abs(frac(x * 0.8) - 0.5) / (0.8 * fwidth(x) + 1e-4);
        float gz = abs(frac(zz * 0.8) - 0.5) / (0.8 * fwidth(zz) + 1e-4);
        float grid = saturate(1.6 - min(gx, gz));
        float fade = exp(-z * 0.08);
        float3 floorc = float3(0.05, 0.0, 0.08);
        float3 lc = lerp(float3(0.9, 0.2, 1.0), float3(0.2, 0.9, 1.0), saturate(-p.y * 2.0));
        col = floorc + lc * grid * fade * (0.7 + 1.3 * bass);
        col += float3(0.6, 0.1, 0.5) * 0.25 * exp(-depth * 18.0);
    }
    // Brume sur l'horizon
    col += float3(0.9, 0.3, 0.6) * 0.18 * exp(-abs(p.y - horizon) * 30.0);
    return float4(col, 1);
}

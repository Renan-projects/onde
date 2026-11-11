// @nom: Encre
// Volutes d'encre colorée qui s'écoulent lentement. La souris remue le
// fluide, les graves accentuent les tourbillons.

float hash21(float2 p) {
    p = frac(p * float2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return frac(p.x * p.y);
}

float noise(float2 p) {
    float2 i = floor(p), f = frac(p);
    float2 u = f * f * (3.0 - 2.0 * f);
    return lerp(lerp(hash21(i), hash21(i + float2(1, 0)), u.x),
                lerp(hash21(i + float2(0, 1)), hash21(i + float2(1, 1)), u.x), u.y);
}

float fbm(float2 p) {
    float v = 0.0, a = 0.5;
    const float2x2 r = float2x2(0.8, 0.6, -0.6, 0.8);
    [unroll] for (int i = 0; i < 4; i++) { v += a * noise(p); p = mul(r, p) * 2.02 + 3.1; a *= 0.5; }
    return v;
}

float4 mainImage(float2 fc) {
    float2 p = (fc - 0.5 * iResolution.xy) / iResolution.y;
    float t = iTime * 0.035 + uEcran * 7.0;

    // La souris creuse un petit tourbillon.
    float2 m = (iMouse.xy - 0.5 * iResolution.xy) / iResolution.y;
    float2 dm = p - m;
    float infl = uPresence * exp(-dot(dm, dm) * 10.0);
    float ang = infl * 1.6;
    float s = sin(ang), c = cos(ang);
    p = m + mul(float2x2(c, -s, s, c), dm);

    p *= 1.6;
    float2 q = float2(fbm(p + t), fbm(p + float2(5.2, 1.3) - t));
    float2 r = float2(fbm(p + 3.5 * q + float2(1.7, 9.2) + t * 1.3), fbm(p + 3.5 * q + float2(8.3, 2.8) - t * 0.9));
    float warp = 3.5 + 2.5 * uSon.x;
    float f = fbm(p + warp * r);

    // Palette « encre » : bleu nuit, sarcelle, magenta, or.
    float3 ink = 0.5 + 0.5 * cos(6.2831 * (f * 0.8 + 0.45 * r.x + t * 0.6 + float3(0.55, 0.70, 0.90)));
    float3 col = lerp(float3(0.01, 0.015, 0.05), ink, smoothstep(0.25, 0.85, f));
    col = lerp(col, float3(0.95, 0.80, 0.55), smoothstep(0.75, 1.0, f) * 0.5);
    col *= 0.55 + 0.6 * length(q);
    col += infl * float3(0.05, 0.04, 0.07);
    // Vignette
    float2 uv = fc / iResolution.xy;
    col *= 0.65 + 0.35 * pow(16.0 * uv.x * uv.y * (1.0 - uv.x) * (1.0 - uv.y), 0.25);
    return float4(col, 1);
}

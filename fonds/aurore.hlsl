// @nom: Aurore boréale
// Rideaux d'aurore au-dessus des montagnes. La souris éclaire le ciel,
// les graves font pulser les rideaux.

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
    [unroll] for (int i = 0; i < 4; i++) { v += a * noise(p); p = p * 2.03 + 17.1; a *= 0.5; }
    return v;
}

float4 mainImage(float2 fc) {
    float2 uv = fc / iResolution.xy;
    float asp = iResolution.x / iResolution.y;
    float2 p = float2(uv.x * asp, uv.y);
    float t = iTime * 0.05 + uEcran * 13.0;

    // Ciel : bleu nuit en haut, plus clair vers l'horizon.
    float3 col = lerp(float3(0.02, 0.07, 0.11), float3(0.003, 0.006, 0.025), pow(uv.y, 0.55));

    // Étoiles
    float2 sg = p * 90.0;
    float2 id = floor(sg);
    float h = hash21(id + uEcran);
    float2 sp = frac(sg) - 0.5 - (float2(hash21(id + 3.1), hash21(id + 7.7)) - 0.5) * 0.6;
    float star = smoothstep(0.09, 0.0, length(sp)) * step(0.982, h) * (0.55 + 0.45 * sin(iTime * (1.0 + h * 3.0) + h * 60.0));
    col += star * 0.9 * smoothstep(0.2, 0.6, uv.y);

    // La souris attire la lumière, les graves la font pulser.
    float2 m = uSouris.x >= 0.0 ? uSouris : float2(0.5, 0.6);
    float near = exp(-pow((p.x - m.x * asp) * 1.6, 2.0)) * uPresence;
    float energy = 0.8 + 0.9 * uSon.x + 0.3 * uSon.w + 0.7 * near;

    float3 acc = 0.0;
    [unroll] for (int i = 0; i < 3; i++) {
        float fi = i;
        float x = p.x * (0.55 + 0.2 * fi) + fi * 2.3;
        float base = 0.40 + 0.13 * fi + 0.30 * (fbm(float2(x * 0.9 + t * (1.0 + 0.5 * fi), fi * 4.1 + t * 0.3)) - 0.5);
        base -= near * 0.04;
        float d = uv.y - base;
        // Stries verticales qui glissent lentement le long du rideau.
        float rays = 0.25 + 0.75 * pow(noise(float2(x * 30.0 + fi * 7.0 + t * 3.0, fi)), 1.5);
        float curtain = exp(-max(d, 0.0) * (3.2 + fi * 1.2)) * exp(min(d, 0.0) * 45.0);
        float3 tint = lerp(float3(0.1, 1.0, 0.45), float3(0.65, 0.2, 1.0), saturate(d * 2.2 + fi * 0.1));
        acc += curtain * rays * tint * (1.0 - 0.25 * fi);
    }
    col += acc * 0.42 * energy;
    col += float3(0.02, 0.14, 0.09) * exp(-uv.y * 5.0) * energy;

    // Montagnes en ombre chinoise, avec un liseré de lumière.
    float ridge = 0.08 + 0.10 * fbm(float2(p.x * 1.3 + uEcran * 5.0, 2.0)) + 0.03 * sin(p.x * 2.1 + 1.0);
    float far = 0.13 + 0.06 * fbm(float2(p.x * 2.5 + 9.0, 5.0));
    if (uv.y < far) col = lerp(col, float3(0.01, 0.025, 0.04), 0.85);
    if (uv.y < ridge) {
        col = float3(0.002, 0.005, 0.01);
        col += float3(0.05, 0.25, 0.18) * 0.25 * energy * exp(-(ridge - uv.y) * 300.0);
    }
    return float4(col, 1);
}

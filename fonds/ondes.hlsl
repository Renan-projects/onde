// @nom: Ondes
// Une eau calme vue de dessus. Cliquer sur le bureau lance des ronds dans
// l'eau, la souris laisse un léger sillage, la musique agite la surface.

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

float height(float2 p) {
    float t = iTime;
    float amp = 1.0 + 1.5 * uSon.x;
    float h = 0.0;
    h += 0.020 * amp * sin(dot(p, float2(0.8, 0.6)) * 7.0 + t * 0.9);
    h += 0.015 * amp * sin(dot(p, float2(-0.5, 0.9)) * 11.0 - t * 1.1);
    h += 0.030 * noise(p * 3.0 + float2(t * 0.15, -t * 0.1));

    // Ronds dans l'eau : un anneau qui s'élargit et s'amortit.
    [unroll] for (int i = 0; i < 4; i++) {
        float age = uClics[i].z;
        if (age < 7.0) {
            float2 c = (uClics[i].xy - 0.5 * iResolution.xy) / iResolution.y;
            float d = length(p - c);
            float front = age * 0.32;
            float env = exp(-pow((d - front) * 7.0, 2.0)) * exp(-age * 0.55) / (1.0 + d * 3.0);
            h += 0.12 * env * sin((d - front) * 70.0);
        }
    }
    // Sillage de la souris
    float2 m = (iMouse.xy - 0.5 * iResolution.xy) / iResolution.y;
    float dm = length(p - m);
    h += 0.03 * uPresence * exp(-dm * dm * 60.0) * sin(dm * 90.0 - t * 6.0);
    return h;
}

float4 mainImage(float2 fc) {
    float2 p = (fc - 0.5 * iResolution.xy) / iResolution.y;
    float e = 1.5 / iResolution.y;
    float h = height(p);
    float2 g = float2(height(p + float2(e, 0)) - h, height(p + float2(0, e)) - h) / e;
    float3 n = normalize(float3(-g * 0.45, 1.0));

    float2 uv = fc / iResolution.xy;
    float3 deep = float3(0.01, 0.10, 0.16);
    float3 shallow = float3(0.05, 0.42, 0.48);
    float3 col = lerp(deep, shallow, smoothstep(-0.3, 1.3, uv.y + 0.4 * uv.x));

    // Éclairage : diffus, reflet spéculaire et reflet du ciel sur les pentes.
    float3 L = normalize(float3(-0.45, 0.5, 0.75));
    col *= 0.55 + 0.65 * saturate(dot(n, L));
    float spec = pow(saturate(dot(reflect(-L, n), float3(0, 0, 1))), 45.0);
    col += float3(0.75, 0.92, 1.0) * spec * 0.55;
    col += float3(0.2, 0.45, 0.5) * saturate(pow(1.0 - n.z, 1.5) * 4.0);

    // Caustiques douces
    float2 cp = p * 5.0 + n.xy * 2.0;
    float ca = abs(sin(cp.x + iTime * 0.4 + sin(cp.y * 1.3 + iTime * 0.3))) * abs(sin(cp.y * 0.9 - iTime * 0.35 + sin(cp.x * 0.7)));
    col += float3(0.10, 0.22, 0.20) * pow(1.0 - ca, 8.0);

    col *= 0.75 + 0.25 * smoothstep(1.2, 0.2, length(uv - 0.5));
    return float4(col, 1);
}

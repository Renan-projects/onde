// @nom: Lampe à lave
// Des bulles de cire qui montent, fusionnent et redescendent. La souris
// attire une bulle, les basses les font gonfler.

float4 mainImage(float2 fc) {
    float2 p = (fc - 0.5 * iResolution.xy) / iResolution.y;
    float asp = iResolution.x / iResolution.y;
    float t = iTime * 0.12 + uEcran * 9.0;
    float grow = 1.0 + 0.25 * uSon.x;

    float field = 0.0;
    [unroll] for (int i = 0; i < 9; i++) {
        float fi = i;
        float2 c;
        c.x = (frac(fi * 0.618) - 0.5) * asp * 0.85 + 0.12 * sin(t * (0.7 + fi * 0.13) + fi * 2.0);
        c.y = 0.62 * sin(t * (0.35 + frac(fi * 0.37) * 0.4) + fi * 1.7);
        float r = (0.09 + 0.07 * frac(fi * 0.73)) * grow;
        float2 d = p - c;
        field += r * r / dot(d, d);
    }
    // Bulle attirée par la souris
    float2 m = (iMouse.xy - 0.5 * iResolution.xy) / iResolution.y;
    float2 dm = p - m;
    field += uPresence * 0.012 / dot(dm, dm);

    float2 uv = fc / iResolution.xy;
    float3 bg = lerp(float3(0.12, 0.02, 0.06), float3(0.03, 0.01, 0.06), uv.y);
    bg += float3(0.25, 0.05, 0.02) * exp(-uv.y * 4.0);

    float edge = smoothstep(0.95, 1.05, field);
    float inner = saturate((field - 1.0) * 0.12);
    float3 wax = lerp(float3(1.0, 0.25, 0.1), float3(1.0, 0.75, 0.25), inner);
    wax = lerp(wax, float3(1.0, 0.35, 0.55), saturate(uv.y * 1.2 - 0.2) * 0.6);
    float3 col = lerp(bg, wax, edge);
    // Halo autour de la cire
    col += float3(0.6, 0.12, 0.08) * saturate(field * 0.35) * (1.0 - edge) * 0.6;
    col *= 0.8 + 0.2 * smoothstep(0.0, 0.5, uv.x) * smoothstep(1.0, 0.5, uv.x);
    return float4(col, 1);
}

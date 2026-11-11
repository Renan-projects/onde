// @nom: Constellation
// Un réseau de points lumineux qui dérivent et se relient. Les points
// proches de la souris s'y connectent, les aigus les font scintiller.

float2 hash22(float2 p) {
    float3 a = frac(p.xyx * float3(123.34, 234.34, 345.65));
    a += dot(a, a + 34.45);
    return frac(float2(a.x * a.y, a.y * a.z));
}

float2 pointAt(float2 cell) {
    float2 n = hash22(cell + uEcran * 17.0);
    return cell + 0.5 + 0.38 * sin(n * 6.2831 + iTime * (0.25 + n.yx * 0.3));
}

float segment(float2 p, float2 a, float2 b, float w) {
    float2 pa = p - a, ba = b - a;
    float h = saturate(dot(pa, ba) / dot(ba, ba));
    return smoothstep(w, w * 0.3, length(pa - ba * h));
}

float4 mainImage(float2 fc) {
    float scale = 9.0;
    float2 p = (fc - 0.5 * iResolution.xy) / iResolution.y * scale;
    float2 m = (iMouse.xy - 0.5 * iResolution.xy) / iResolution.y * scale;
    float w = 1.6 * scale / iResolution.y;

    float2 uv = fc / iResolution.xy;
    float3 col = lerp(float3(0.01, 0.015, 0.04), float3(0.03, 0.02, 0.07), uv.y);
    col += float3(0.05, 0.02, 0.08) * exp(-length(uv - float2(0.3, 0.7)) * 2.0);

    float2 cell = floor(p);
    float2 pts[9];
    [unroll] for (int j = 0; j < 9; j++) {
        pts[j] = pointAt(cell + float2(j % 3 - 1, j / 3 - 1));
    }
    float3 lineCol = float3(0.35, 0.6, 1.0);
    float glowSum = 0.0;
    [unroll] for (int i = 0; i < 9; i++) {
        float2 a = pts[i];
        // Lien avec le point central (indice 4).
        if (i != 4) {
            float alpha = smoothstep(1.5, 0.6, length(a - pts[4]));
            col += lineCol * segment(p, pts[4], a, w) * alpha * 0.35;
        }
        float d = length(p - a);
        float tw = 0.6 + 0.4 * sin(iTime * 2.0 + dot(a, float2(3.1, 7.7)) + uSon.z * 6.0);
        glowSum += (0.025 / (d * d * 60.0 + 0.04)) * tw * (1.0 + uSon.z * 1.5);
        // Lien vers la souris
        float near = smoothstep(2.6, 0.8, length(a - m)) * uPresence;
        col += float3(0.9, 0.5, 1.0) * segment(p, a, m, w) * near * 0.5;
    }
    // Liens transverses pour casser l'effet de grille
    col += lineCol * segment(p, pts[1], pts[3], w) * smoothstep(1.5, 0.6, length(pts[1] - pts[3])) * 0.25;
    col += lineCol * segment(p, pts[1], pts[5], w) * smoothstep(1.5, 0.6, length(pts[1] - pts[5])) * 0.25;
    col += lineCol * segment(p, pts[7], pts[3], w) * smoothstep(1.5, 0.6, length(pts[7] - pts[3])) * 0.25;
    col += lineCol * segment(p, pts[7], pts[5], w) * smoothstep(1.5, 0.6, length(pts[7] - pts[5])) * 0.25;

    col += float3(0.6, 0.8, 1.0) * saturate(glowSum);
    float dm = length(p - m);
    col += float3(0.7, 0.4, 1.0) * uPresence * 0.08 / (dm * dm + 0.3);
    return float4(col, 1);
}

package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.Paint

/**
 * The world clouds' two flow layers in one pass. A platform that can run a runtime shader sets
 * [MeshCanvas.cloudFlow]; the clouds then draw one textured mesh through [paint] instead of one per
 * layer and shadow. The shader slides the second layer along the wind, weighs both layers and fades
 * them towards the poles exactly as the mesh path does, so the two look the same.
 */
interface CloudFlow {
    /** What the mesh is drawn with; its shader is replaced by each [set]. */
    val paint: Paint

    /**
     * [strength] is the cloud setting times the zoom fade, [w0] and [w1] the two layers' weights, [slide] the seconds of wind
     * between the layers, [tw] and [th] the texture's texels a degree across and down, [shadow] a pass of shadow instead of cloud.
     */
    fun set(strength: Float, w0: Float, w1: Float, slide: Float, tw: Float, th: Float, shadow: Boolean)
}

/** The shader's source, in the dialect Skia (desktop) and Android 13 (AGSL) share. */
object CloudSksl {
    const val SOURCE = """
uniform shader tex;
uniform float strength;
uniform float w0;
uniform float w1;
uniform float slide;
uniform float tw;
uniform float th;
uniform float shadow;

float bump(float a, float centre, float width) {
    float x = (a - centre) / width;
    return exp(-x * x);
}

float wind(float lat) {
    float a = abs(lat);
    return -0.05 * bump(a, 14.0, 12.0) + 0.10 * bump(a, 47.0, 13.0) - 0.02 * bump(a, 75.0, 8.0);
}

half4 main(float2 uv) {
    float lat = 90.0 - uv.y / th;
    float k = clamp((abs(lat) - 84.0) / (70.0 - 84.0), 0.0, 1.0);
    float pole = k * k * (3.0 - 2.0 * k);
    float a0 = strength * w0 * pole;
    float a1 = strength * w1 * pole;
    half4 c0 = tex.eval(uv);
    half4 c1 = tex.eval(float2(uv.x + (-wind(lat) * slide + 133.0) * tw, uv.y));
    if (shadow > 0.5) {
        float s0 = c0.a * 0.14 * a0;
        float s1 = c1.a * 0.14 * a1;
        return half4(0.0, 0.0, 0.0, 1.0 - (1.0 - s0) * (1.0 - s1));
    }
    half4 p0 = c0 * a0;
    half4 p1 = c1 * a1;
    return p1 + p0 * (1.0 - p1.a);
}
"""
}

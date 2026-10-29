package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.asSkiaBitmap
import org.jetbrains.skia.FilterTileMode
import org.jetbrains.skia.RuntimeEffect
import org.jetbrains.skia.RuntimeShaderBuilder
import org.jetbrains.skia.SamplingMode

private val cloudEffect: RuntimeEffect? by lazy { runCatching { RuntimeEffect.makeForShader(CloudSksl.SOURCE) }.getOrNull() }

/** The clouds' flow layers as one Skia runtime shader over the cloud texture; null if the shader will not compile, which keeps the mesh path. */
internal val desktopCloudFlow: (ImageBitmap) -> CloudFlow? = { image ->
    val effect = cloudEffect
    if (effect == null) null else {
        val texture = org.jetbrains.skia.Image.makeFromBitmap(image.asSkiaBitmap()).makeShader(FilterTileMode.REPEAT, FilterTileMode.CLAMP, SamplingMode.LINEAR, null)
        object : CloudFlow {
            override val paint = Paint()
            override fun set(strength: Float, w0: Float, w1: Float, slide: Float, tw: Float, th: Float, shadow: Boolean) {
                val builder = RuntimeShaderBuilder(effect)
                try {
                    builder.child("tex", texture)
                    builder.uniform("strength", strength); builder.uniform("w0", w0); builder.uniform("w1", w1)
                    builder.uniform("slide", slide); builder.uniform("tw", tw); builder.uniform("th", th)
                    builder.uniform("shadow", if (shadow) 1f else 0f)
                    paint.shader = builder.makeShader(null)
                } finally {
                    builder.close()
                }
            }
        }
    }
}

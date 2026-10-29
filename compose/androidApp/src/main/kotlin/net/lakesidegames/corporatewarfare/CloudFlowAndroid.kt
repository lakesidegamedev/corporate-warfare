package net.lakesidegames.corporatewarfare

import android.graphics.BitmapShader
import android.graphics.RuntimeShader
import android.graphics.Shader
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.asAndroidBitmap

/**
 * The clouds' flow layers as one AGSL runtime shader over the cloud texture, on Android 13 and up. Null if the shader will not
 * build, which keeps the mesh path; below Android 13 it is never installed.
 */
@RequiresApi(Build.VERSION_CODES.TIRAMISU)
internal fun androidCloudFlow(image: ImageBitmap): CloudFlow? = runCatching {
    val texture = BitmapShader(image.asAndroidBitmap(), Shader.TileMode.REPEAT, Shader.TileMode.CLAMP).apply { filterMode = BitmapShader.FILTER_MODE_LINEAR }
    // Built once here so a source the device rejects is found now, not in the middle of a frame.
    RuntimeShader(CloudSksl.SOURCE)
    object : CloudFlow {
        override val paint = Paint()
        override fun set(strength: Float, w0: Float, w1: Float, slide: Float, tw: Float, th: Float, shadow: Boolean) {
            // A shader per pass: a recorded draw keeps its uniforms, so two passes never share one.
            val shader = RuntimeShader(CloudSksl.SOURCE)
            shader.setInputShader("tex", texture)
            shader.setFloatUniform("strength", strength)
            shader.setFloatUniform("w0", w0)
            shader.setFloatUniform("w1", w1)
            shader.setFloatUniform("slide", slide)
            shader.setFloatUniform("tw", tw)
            shader.setFloatUniform("th", th)
            shader.setFloatUniform("shadow", if (shadow) 1f else 0f)
            paint.shader = shader
        }
    }
}.getOrNull()

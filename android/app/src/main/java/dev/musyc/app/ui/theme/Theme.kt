package dev.musyc.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val Brand = Color(0xFF12AA9C)
private val BrandDark = Color(0xFF0B6E66)

private val LightColors = lightColorScheme(
    primary = BrandDark,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFB2F0E8),
    onPrimaryContainer = Color(0xFF00211D),
    secondary = Color(0xFF4A635F),
    surface = Color(0xFFFAFDFB),
    background = Color(0xFFFAFDFB),
)

private val DarkColors = darkColorScheme(
    primary = Brand,
    onPrimary = Color(0xFF003733),
    primaryContainer = Color(0xFF00504A),
    onPrimaryContainer = Color(0xFFB2F0E8),
    secondary = Color(0xFFB1CCC7),
    surface = Color(0xFF101413),
    background = Color(0xFF101413),
)

@Composable
fun MusycTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    // 固定使用品牌色，跟随系统明暗；不使用 Material You 动态取色。
    val scheme = if (darkTheme) DarkColors else LightColors
    MaterialTheme(colorScheme = scheme, content = content)
}

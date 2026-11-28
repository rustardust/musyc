package dev.musyc.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.SideEffect
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.lifecycle.viewmodel.compose.viewModel
import dev.musyc.app.play.PlaybackService
import dev.musyc.app.ui.AppViewModel
import dev.musyc.app.ui.MusycRoot
import dev.musyc.app.ui.theme.MusycTheme

class MainActivity : ComponentActivity() {
    private val notificationPermission = registerForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        // 前台启动媒体服务：MediaSession 建立后系统媒体通知/锁屏控制才能出现。
        runCatching { startService(Intent(this, PlaybackService::class.java)) }
        setContent {
            MusycTheme {
                val dark = isSystemInDarkTheme()
                SideEffect {
                    WindowCompat.getInsetsController(window, window.decorView).apply {
                        isAppearanceLightStatusBars = !dark
                        isAppearanceLightNavigationBars = !dark
                    }
                }
                val vm: AppViewModel = viewModel()
                MusycRoot(vm)
            }
        }
    }
}

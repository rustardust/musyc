package dev.musyc.app

import android.app.Application
import dev.musyc.app.play.PlaybackManager

class MusycApp : Application() {
    override fun onCreate() {
        super.onCreate()
        PlaybackManager.init(this)
    }
}

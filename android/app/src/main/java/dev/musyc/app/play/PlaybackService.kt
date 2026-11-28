package dev.musyc.app.play

import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        PlaybackManager.init(this)
        session = MediaSession.Builder(this, PlaybackManager.player).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        session?.run {
            player.pause()
            release()
        }
        session = null
        super.onDestroy()
    }
}

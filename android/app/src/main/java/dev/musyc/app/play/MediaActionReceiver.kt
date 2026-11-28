package dev.musyc.app.play

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** 媒体通知按钮的广播入口（通知里的上一首 / 播放暂停 / 下一首）。 */
class MediaActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            ACTION_TOGGLE -> PlaybackManager.toggle()
            ACTION_PREV -> PlaybackManager.previous()
            ACTION_NEXT -> PlaybackManager.next()
        }
    }

    companion object {
        const val ACTION_TOGGLE = "dev.musyc.app.MEDIA_TOGGLE"
        const val ACTION_PREV = "dev.musyc.app.MEDIA_PREV"
        const val ACTION_NEXT = "dev.musyc.app.MEDIA_NEXT"
    }
}

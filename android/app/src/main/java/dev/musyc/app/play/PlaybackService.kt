package dev.musyc.app.play

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.media3.common.Player
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.MediaStyleNotificationHelper
import dev.musyc.app.MainActivity
import dev.musyc.app.R

class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        PlaybackManager.init(this)
        val openApp = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaSession.Builder(this, PlaybackManager.player)
            .setSessionActivity(openApp)
            .build()
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "播放控制", NotificationManager.IMPORTANCE_LOW),
        )
        PlaybackManager.player.addListener(object : Player.Listener {
            override fun onEvents(player: Player, events: Player.Events) {
                updateNotification()
            }
        })
    }

    private fun updateNotification() {
        val session = session ?: return
        val player = session.player
        val item = player.currentMediaItem ?: return
        val meta = item.mediaMetadata
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_music)
            .setContentTitle(meta.title?.toString().orEmpty().ifEmpty { getString(R.string.app_name) })
            .setContentText(meta.artist?.toString().orEmpty())
            .setContentIntent(session.sessionActivity)
            .setStyle(MediaStyleNotificationHelper.MediaStyle(session))
            .setOngoing(player.isPlaying)
            .setSilent(true)
            .setShowWhen(false)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .build()
        if (player.isPlaying) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
            } else {
                startForeground(NOTIFICATION_ID, notification)
            }
        } else {
            getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification)
        }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /**
     * 自行接管媒体通知：Media3 默认 provider 在此工程里未 post 通知，
     * 这里用 MediaStyle + 会话 token 生成系统媒体卡片（Android 13+ 的控制按钮由会话推导）。
     */
    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        val player = session.player
        if (player.currentMediaItem == null) {
            stopForeground(STOP_FOREGROUND_REMOVE)
            return
        }
        val meta = player.mediaMetadata
        val title = meta.title?.toString().orEmpty()
        val artist = meta.artist?.toString().orEmpty()
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_music)
            .setContentTitle(title.ifEmpty { getString(R.string.app_name) })
            .setContentText(artist)
            .setContentIntent(session.sessionActivity)
            .setStyle(MediaStyleNotificationHelper.MediaStyle(session))
            .setOngoing(player.isPlaying)
            .setSilent(true)
            .setShowWhen(false)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .build()
        if (startInForegroundRequired) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
            } else {
                startForeground(NOTIFICATION_ID, notification)
            }
        }
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification)
    }

    override fun onDestroy() {
        session?.run {
            player.pause()
            release()
        }
        session = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        super.onDestroy()
    }

    private companion object {
        const val CHANNEL_ID = "musyc-playback"
        const val NOTIFICATION_ID = 1
    }
}

package dev.musyc.app.play

import android.content.Context
import android.os.SystemClock
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import dev.musyc.app.data.Repository
import dev.musyc.app.data.Song
import dev.musyc.app.data.Source
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

enum class PlayMode(val label: String) {
    Sequential("顺序"),
    ListLoop("列表循环"),
    Single("单曲"),
    Shuffle("随机"),
}

data class PlaybackUiState(
    val queue: List<Song> = emptyList(),
    val index: Int = -1,
    val song: Song? = null,
    val playing: Boolean = false,
    val buffering: Boolean = false,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
    val sleepLeftMs: Long? = null,
    val message: String? = null,
    val mode: PlayMode = PlayMode.ListLoop,
    val volume: Float = 1f,
)

/**
 * 进程内播放核心：ExoPlayer + 懒解析队列。
 * 队列里的地址按需解析（网易直链有时效性），解析失败自动跳到下一首。
 */
object PlaybackManager {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val _state = MutableStateFlow(PlaybackUiState())
    val state: StateFlow<PlaybackUiState> = _state.asStateFlow()

    lateinit var player: ExoPlayer
        private set

    private var loadJob: Job? = null
    private var started = false
    private lateinit var prefs: android.content.SharedPreferences
    private var sleepDeadline: Long? = null

    fun init(context: Context) {
        if (started) return
        started = true
        prefs = context.getSharedPreferences("musyc-play", Context.MODE_PRIVATE)
        val savedMode = prefs.getString("mode", null)
            ?.let { name -> PlayMode.entries.firstOrNull { it.name == name } }
            ?: PlayMode.ListLoop
        _state.update { it.copy(mode = savedMode, volume = prefs.getFloat("volume", 1f)) }
        player = ExoPlayer.Builder(context.applicationContext).build().apply {
            setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                    .build(),
                true,
            )
            setHandleAudioBecomingNoisy(true)
            volume = prefs.getFloat("volume", 1f)
            repeatMode = Player.REPEAT_MODE_OFF
            addListener(object : Player.Listener {
                override fun onPlaybackStateChanged(playbackState: Int) {
                    when (playbackState) {
                        Player.STATE_BUFFERING -> _state.update { it.copy(buffering = true) }
                        Player.STATE_READY -> {
                            val d = duration
                            _state.update { st ->
                                st.copy(
                                    buffering = false,
                                    durationMs = if (d > 0) d else st.durationMs,
                                )
                            }
                        }
                        Player.STATE_ENDED -> PlaybackManager.next(auto = true)
                        else -> Unit
                    }
                }

                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    _state.update { it.copy(playing = isPlaying) }
                }

                override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                    _state.update { it.copy(message = "播放失败") }
                    PlaybackManager.next(auto = true)
                }
            })
        }
        scope.launch {
            while (true) {
                delay(500)
                val st = _state.value
                val pos = if (player.currentMediaItem != null) player.currentPosition else 0L
                val dur = if (player.duration > 0) player.duration else st.durationMs
                val left = sleepDeadline?.let { (it - SystemClock.elapsedRealtime()).coerceAtLeast(0) }
                if (left != null && left <= 0) {
                    sleepDeadline = null
                    player.pause()
                    _state.update { it.copy(message = "定时结束", sleepLeftMs = null) }
                } else if (left != null && st.sleepLeftMs != left) {
                    _state.update { it.copy(sleepLeftMs = left) }
                }
                if (pos != st.positionMs || dur != st.durationMs) {
                    _state.update { it.copy(positionMs = pos, durationMs = dur) }
                }
            }
        }
    }

    fun play(queue: List<Song>, index: Int) {
        if (queue.isEmpty() || index !in queue.indices) return
        _state.update { it.copy(queue = queue, index = index, song = queue[index]) }
        load(index)
    }

    fun toggle() {
        val st = _state.value
        if (st.song == null) {
            if (st.queue.isNotEmpty()) load(st.index.coerceAtLeast(0)) else return
            return
        }
        if (!player.isPlaying) {
            player.play()
        } else {
            player.pause()
        }
    }

    /** auto=true 表示由播放结束触发：单曲重播、顺序模式播完即停。 */
    fun next(auto: Boolean = false) {
        val st = _state.value
        if (st.queue.isEmpty()) return
        val size = st.queue.size
        val target = when (st.mode) {
            PlayMode.Single -> if (auto) st.index else (st.index + 1) % size
            PlayMode.Shuffle -> if (size == 1) 0 else {
                var n = st.index
                while (n == st.index) n = kotlin.random.Random.nextInt(size)
                n
            }
            PlayMode.Sequential -> {
                if (auto && st.index == size - 1) {
                    player.pause()
                    _state.update { it.copy(playing = false, message = "队列结束") }
                    return
                }
                (st.index + 1) % size
            }
            PlayMode.ListLoop -> (st.index + 1) % size
        }
        _state.update { it.copy(index = target, song = st.queue[target]) }
        load(target)
    }

    fun cycleMode() {
        val st = _state.value
        val mode = PlayMode.entries[(st.mode.ordinal + 1) % PlayMode.entries.size]
        prefs.edit().putString("mode", mode.name).apply()
        _state.update { it.copy(mode = mode, message = mode.label) }
    }

    fun previous() {
        val st = _state.value
        if (st.queue.isEmpty()) return
        val prev = if (st.index <= 0) st.queue.size - 1 else st.index - 1
        _state.update { it.copy(index = prev, song = st.queue[prev]) }
        load(prev)
    }

    fun seekTo(ms: Long) {
        if (player.currentMediaItem != null) player.seekTo(ms.coerceAtLeast(0))
    }

    /** 对应 TUI 的 ←/→：±5 秒。 */
    fun seekBy(deltaMs: Long) {
        if (player.currentMediaItem == null) return
        val duration = if (player.duration > 0) player.duration else _state.value.durationMs
        val target = (player.currentPosition + deltaMs).coerceIn(0, duration.coerceAtLeast(0))
        player.seekTo(target)
        _state.update { it.copy(positionMs = target) }
    }

    fun setVolume(value: Float) {
        val v = value.coerceIn(0f, 1f)
        if (::player.isInitialized) player.volume = v
        prefs.edit().putFloat("volume", v).apply()
        _state.update { it.copy(volume = v) }
    }

    /** minutes <= 0 表示取消。 */
    fun setSleep(minutes: Int) {
        sleepDeadline = if (minutes <= 0) null else SystemClock.elapsedRealtime() + minutes * 60_000L
        _state.update {
            it.copy(
                sleepLeftMs = sleepDeadline?.minus(SystemClock.elapsedRealtime()),
                message = if (minutes <= 0) "定时取消" else "定时 $minutes 分",
            )
        }
    }

    fun consumeMessage() {
        _state.update { it.copy(message = null) }
    }

    private fun load(index: Int) {
        loadJob?.cancel()
        val queue = _state.value.queue
        val song = queue.getOrNull(index) ?: return
        loadJob = scope.launch {
            _state.update { it.copy(buffering = true, durationMs = song.durationMs) }
            var tries = 0
            var cursor = index
            var current = song
            // 与 TUI 一致：连续失败超过 8 首就停下，避免整张 VIP 歌单被逐首跳过。
            while (tries < queue.size.coerceAtMost(8)) {
                val resolved = withContext(Dispatchers.IO) { Repository.resolve(current) }
                val playable = resolved != null &&
                    if (current.source == dev.musyc.app.data.Source.LOCAL) {
                        !resolved.localUri.isNullOrBlank()
                    } else {
                        resolved.playUrl?.startsWith("http") == true
                    }
                if (playable) {
                    resolved ?: continue
                    val uri = if (current.source == Source.LOCAL) current.localUri else resolved.playUrl
                    player.setMediaItem(buildItem(resolved, uri!!))
                    player.prepare()
                    player.play()
                    _state.update {
                        it.copy(
                            song = resolved,
                            index = cursor,
                            buffering = false,
                            playing = true,
                            positionMs = 0,
                        )
                    }
                    return@launch
                }
                tries++
                cursor = (cursor + 1) % queue.size
                current = queue[cursor]
            }
            _state.update { it.copy(buffering = false, playing = false, message = "无可播放") }
        }
    }

    private fun buildItem(song: Song, uri: String): MediaItem {
        val metadata = MediaMetadata.Builder()
            .setTitle(song.title)
            .setArtist(song.artists)
            .setAlbumTitle(song.album)
            .setArtworkUri(song.coverUrl?.let(android.net.Uri::parse))
            .build()
        return MediaItem.Builder()
            .setMediaId(song.key)
            .setUri(uri)
            .setMediaMetadata(metadata)
            .build()
    }
}

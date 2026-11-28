package dev.musyc.app.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** 各来源的统一入口，UI 只跟这里打交道。 */
object Repository {
    @Volatile private var chartCache: Playlist? = null
    @Volatile private var streetCache: Playlist? = null
    @Volatile private var indieCache: List<Playlist>? = null

    suspend fun neteaseChart(force: Boolean = false): Playlist = withContext(Dispatchers.IO) {
        chartCache?.takeUnless { force }?.let { return@withContext it }
        val songs = NeteaseApi.chart()
        Playlist(
            id = "netease-chart",
            title = "网易原创",
            subtitle = "网易原创榜 · 官方直链",
            coverUrl = songs.firstOrNull()?.coverUrl,
            songs = songs,
        ).also { chartCache = it }
    }

    suspend fun streetVoice(force: Boolean = false): Playlist = withContext(Dispatchers.IO) {
        streetCache?.takeUnless { force }?.let { return@withContext it }
        val songs = StreetVoiceApi.recommend()
        Playlist(
            id = "streetvoice",
            title = "街声推荐",
            subtitle = "${songs.size} 首 · 街声",
            coverUrl = songs.firstOrNull()?.coverUrl,
            songs = songs,
        ).also { streetCache = it }
    }

    suspend fun indie(force: Boolean = false): List<Playlist> = withContext(Dispatchers.IO) {
        indieCache?.takeUnless { force }?.let { return@withContext it }
        IndieApi.catalog().also { indieCache = it }
    }

    /** 播放前解析出可播放地址。拿不到官方直链时返回 null，由调用方跳过。 */
    suspend fun resolve(song: Song): Song? = withContext(Dispatchers.IO) {
        when (song.source) {
            Source.LOCAL -> song
            Source.INDIE, Source.AUDIUS, Source.CUSTOM -> song.takeIf { !it.playUrl.isNullOrBlank() }
            Source.STREETVOICE -> StreetVoiceApi.prepare(song)
            Source.NETEASE -> NeteaseApi.streamUrl(song.id)?.let { url ->
                song.copy(playUrl = url)
            }
        }
    }

    suspend fun lyric(song: Song): Lyric = withContext(Dispatchers.IO) {
        when (song.source) {
            Source.NETEASE -> NeteaseApi.lyric(song.id)
            Source.STREETVOICE -> StreetVoiceApi.lyric(song)
            Source.INDIE -> IndieApi.lyric(song)
            else -> Lyric(emptyList())
        }
    }
}

package dev.musyc.app.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.util.concurrent.TimeUnit

/**
 * 街声（streetvoice.cn）目录与播放。
 *
 * 列表来自服务端渲染的 HTMX 片段；播放地址是 HLS（m3u8），交给 ExoPlayer 直接播放，
 * 不再像桌面端那样用 ffmpeg 转 MP3。
 */
object StreetVoiceApi {
    private const val BASE = "https://www.streetvoice.cn"
    private const val UA =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
            "(KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"
    private const val LIMIT = 50
    private const val MAX_PAGES = 10

    private val http = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(20, TimeUnit.SECONDS)
        .build()

    suspend fun recommend(): List<Song> = withContext(Dispatchers.IO) { list("0") }

    suspend fun category(cat: String): List<Song> = withContext(Dispatchers.IO) { list(cat) }

    private fun list(cat: String): List<Song> {
        val songs = LinkedHashMap<String, Song>()
        for (page in 0 until MAX_PAGES) {
            val query = if (page > 0) "?page=${page + 1}" else ""
            val html = get("$BASE/music/browse/$cat/recommend/latest/$query", htmx = true)
            val parsed = parseList(html)
            if (parsed.isEmpty()) break
            parsed.forEach { songs.putIfAbsent(it.id, it) }
            if (songs.size >= LIMIT) break
        }
        return songs.values.take(LIMIT)
    }

    private fun parseList(html: String): List<Song> {
        val chunkRe = Regex("<li class=\"list-group-item[^\"]*work-item[^\"]*\"")
        val starts = chunkRe.findAll(html).map { it.range.first }.toList()
        if (starts.isEmpty()) return emptyList()
        val out = mutableListOf<Song>()
        starts.forEachIndexed { index, start ->
            val end = starts.getOrNull(index + 1) ?: html.length
            val chunk = html.substring(start, end)
            val id = Regex("data-id=\"(\\d+)\"").find(chunk)?.groupValues?.get(1) ?: return@forEachIndexed
            val title = heading(chunk, "h4") ?: return@forEachIndexed
            val artist = heading(chunk, "h5").orEmpty()
            val cover = Regex("<img\\s+src=\"([^\"]+)\"").find(chunk)?.groupValues?.get(1)
            out.add(
                Song(
                    id = id,
                    title = title,
                    artists = artist,
                    coverUrl = cover,
                    source = Source.STREETVOICE,
                )
            )
        }
        return out
    }

    private fun heading(chunk: String, tag: String): String? {
        val block = Regex("<$tag\\b[^>]*class=\"[^\"]*text-truncate[^\"]*\"[^>]*>([\\s\\S]*?)</$tag>")
            .find(chunk)?.groupValues?.get(1) ?: return null
        val anchor = Regex("<a\\b[^>]*>([\\s\\S]*?)</a>").find(block)?.groupValues?.get(1) ?: block
        return stripTags(anchor).takeIf(String::isNotBlank)
    }

    /** 播放前补时长 / 专辑 / 歌词，并拿到 HLS 地址。 */
    suspend fun prepare(song: Song): Song? = withContext(Dispatchers.IO) {
        val detail = runCatching { get("$BASE/api/v5/song/${song.id}/") }.getOrNull() ?: return@withContext null
        val json = JSONObject(detail)
        val lyrics = json.optString("lyrics")
        val hls = runCatching { hlsUrl(song.id) }.getOrNull() ?: return@withContext null
        song.copy(
            title = json.optString("name").ifBlank { song.title },
            album = json.optJSONObject("album")?.optString("name").orEmpty(),
            durationMs = (json.optDouble("length", 0.0) * 1000).toLong(),
            coverUrl = json.optString("image").ifBlank { song.coverUrl },
            playUrl = hls,
        ).also {
            lyricCache[song.id] = json.optString("lyrics_is_lrc").toBoolean()
            lyricsCache[song.id] = lyrics
        }
    }

    private val lyricsCache = mutableMapOf<String, String>()
    private val lyricCache = mutableMapOf<String, Boolean>()

    suspend fun lyric(song: Song): Lyric = withContext(Dispatchers.IO) {
        if (!lyricsCache.containsKey(song.id)) prepare(song)
        val text = lyricsCache[song.id].orEmpty()
        if (text.isBlank()) return@withContext Lyric(emptyList())
        if (lyricCache[song.id] == true) LyricParser.parse(text) else Lyric(emptyList(), text.trim())
    }

    private fun hlsUrl(id: String): String {
        val request = Request.Builder()
            .url("$BASE/api/v5/song/$id/hls/file/")
            .header("User-Agent", UA)
            .header("Referer", "$BASE/music/charts/realtime/all/")
            .header("X-Requested-With", "XMLHttpRequest")
            .post("".toRequestBody("application/x-www-form-urlencoded".toMediaType()))
            .build()
        http.newCall(request).execute().use { resp ->
            val text = resp.body?.string().orEmpty()
            val file = JSONObject(text).optString("file")
            if (file.isBlank()) error("街声没有返回 HLS 地址")
            return file
        }
    }

    private fun get(url: String, htmx: Boolean = false): String {
        val builder = Request.Builder()
            .url(url)
            .header("User-Agent", UA)
            .header("Accept", "text/html,application/json")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        if (htmx) builder.header("HX-Request", "true")
        http.newCall(builder.build()).execute().use { resp ->
            if (!resp.isSuccessful) error("街声 HTTP ${resp.code}")
            return resp.body?.string().orEmpty()
        }
    }

    private fun stripTags(text: String): String =
        text.replace(Regex("<[^>]*>"), "")
            .replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
            .replace("&quot;", "\"").replace("&#39;", "'").replace("&#x27;", "'")
            .replace(Regex("&#(\\d+);")) { String(Character.toChars(it.groupValues[1].toInt())) }
            .replace(Regex("\\s+"), " ")
            .trim()
}

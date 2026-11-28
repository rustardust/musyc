package dev.musyc.app.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.TimeUnit

/** 雀乐（indie.cn）歌单目录：首页找最新 vol，再逐期抓 Next.js flight 数据。 */
object IndieApi {
    private const val HOME = "https://www.indie.cn/"
    private const val COUNT = 20
    private const val UA =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
            "(KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"

    private val http = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(20, TimeUnit.SECONDS)
        .build()

    suspend fun catalog(): List<Playlist> = withContext(Dispatchers.IO) {
        val home = fetch(HOME)
        val ids = volumeIds(parseFlight(home)).take(COUNT)
        if (ids.isEmpty()) error("雀乐首页没有找到歌单")
        coroutineScope {
            ids.map { id -> async { runCatching { loadVolume(id) }.getOrNull() } }.awaitAll()
        }.filterNotNull()
    }

    suspend fun lyric(song: Song): Lyric = withContext(Dispatchers.IO) {
        val url = song.lyricUrl ?: return@withContext Lyric(emptyList())
        val text = runCatching { fetch(url) }.getOrNull().orEmpty()
        LyricParser.parse(text)
    }

    private fun loadVolume(id: String): Playlist? {
        val html = fetch("https://www.indie.cn/vol/$id")
        val flight = parseFlight(html)
        val arrayText = extractArrayAfter(flight, "\"songList\":") ?: return null
        val raw = JSONArray(arrayText)
        val songs = mutableListOf<Song>()
        for (i in 0 until raw.length()) {
            songFrom(raw.optJSONObject(i))?.let(songs::add)
        }
        if (songs.isEmpty()) return null
        val title = Regex("<title>([^<]*)</title>").find(html)?.groupValues?.get(1)
            ?.let(::stripTags)
            ?.replace(Regex("\\s*[-–—]\\s*雀乐\\s*$"), "")
            ?.trim()
            ?.takeIf(String::isNotBlank)
            ?: "Vol $id"
        return Playlist(
            id = id,
            title = title,
            subtitle = "${songs.size} 首",
            coverUrl = songs.first().coverUrl,
            songs = songs,
        )
    }

    private fun songFrom(o: JSONObject?): Song? {
        if (o == null) return null
        val title = o.optString("title").takeIf(String::isNotBlank) ?: return null
        val id = o.optString("id").ifBlank {
            "${o.optString("journalNo")}_${o.optString("songNo")}"
        }
        return Song(
            id = id,
            title = title,
            artists = o.optString("artist"),
            album = o.optString("album"),
            durationMs = parseDuration(o.opt("duration")),
            coverUrl = o.optString("pic").takeIf { it.startsWith("http") },
            source = Source.INDIE,
            playUrl = o.optString("src").takeIf { it.startsWith("http") },
            lyricUrl = o.optString("lrc").takeIf { it.startsWith("http") },
        )
    }

    private fun parseDuration(raw: Any?): Long = when (raw) {
        is Number -> if (raw.toLong() > 1000) raw.toLong() else raw.toLong() * 1000
        is String -> {
            val parts = raw.trim().split(":")
            if (parts.size == 2) {
                (parts[0].toLongOrNull() ?: 0) * 60_000 + (parts[1].toLongOrNull() ?: 0) * 1000
            } else 0
        }
        else -> 0
    }

    private fun volumeIds(flight: String): List<String> =
        Regex("\"href\":\"/vol/(\\d+)\"").findAll(flight)
            .map { it.groupValues[1].toInt() }
            .filter { it > 0 }
            .distinct()
            .sortedDescending()
            .map(Int::toString)
            .toList()

    private fun parseFlight(html: String): String {
        val re = Regex("self\\.__next_f\\.push\\(\\[1,(\"(?:[^\"\\\\]|\\\\.)*\")\\]\\)")
        val out = StringBuilder()
        re.findAll(html).forEach { match ->
            runCatching { out.append(JSONArray("[${match.groupValues[1]}]").getString(0)) }
        }
        return out.toString()
    }

    private fun extractArrayAfter(text: String, marker: String): String? {
        val at = text.indexOf(marker)
        if (at < 0) return null
        val start = text.indexOf('[', at + marker.length)
        if (start < 0) return null
        var depth = 0
        var inString = false
        var escaped = false
        for (i in start until text.length) {
            val ch = text[i]
            if (inString) {
                when {
                    escaped -> escaped = false
                    ch == '\\' -> escaped = true
                    ch == '"' -> inString = false
                }
                continue
            }
            when (ch) {
                '"' -> inString = true
                '[' -> depth++
                ']' -> {
                    depth--
                    if (depth == 0) return text.substring(start, i + 1)
                }
            }
        }
        return null
    }

    private fun fetch(url: String): String {
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", UA)
            .header("Accept", "text/html,application/xhtml+xml")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .build()
        http.newCall(request).execute().use { resp ->
            if (!resp.isSuccessful) error("雀乐 HTTP ${resp.code}")
            return resp.body?.string().orEmpty()
        }
    }

    private fun stripTags(text: String): String =
        text.replace(Regex("<[^>]*>"), "")
            .replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
            .replace("&quot;", "\"").replace("&#39;", "'").replace("&#x27;", "'")
            .replace(Regex("\\s+"), " ")
            .trim()
}

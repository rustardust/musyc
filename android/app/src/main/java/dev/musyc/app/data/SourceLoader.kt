package dev.musyc.app.data

import android.text.Html
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.TimeUnit

/** 在线歌单源统一加载入口（对应桌面端 source_loader.rs）。 */
object SourceLoader {
    private val http = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(20, TimeUnit.SECONDS)
        .build()

    private const val UA =
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36"

    suspend fun load(def: SourceDef): List<Song> = withContext(Dispatchers.IO) {
        when (def.type) {
            SourceType.NETEASE_CHART -> NeteaseApi.chart()
            SourceType.NETEASE_PLAYLIST -> NeteaseApi.playlistSongs(def.playlistId)
            SourceType.STREETVOICE_RECOMMEND -> StreetVoiceApi.recommend()
            SourceType.STREETVOICE_CAT -> StreetVoiceApi.category(def.cat)
            SourceType.INDIE -> throw IllegalStateException("雀乐按期浏览")
            SourceType.AUDIUS -> audius(def)
            SourceType.FMA -> fma(def)
            SourceType.JAMENDO -> jamendo(def)
            SourceType.GENERIC -> generic(def)
        }
    }

    private fun audius(def: SourceDef): List<Song> {
        val url = "https://api.audius.co/v1/tracks/trending?time=${def.time}&app_name=musyc&limit=${def.limit}"
        val data = getJson(url).optJSONArray("data") ?: JSONArray()
        return (0 until data.length()).mapNotNull { i ->
            val o = data.optJSONObject(i) ?: return@mapNotNull null
            val id = o.optString("id").ifBlank { return@mapNotNull null }
            val user = o.optJSONObject("user")?.optString("name").orEmpty().ifBlank { "Audius" }
            Song(
                id = id,
                title = o.optString("title", "未命名"),
                artists = user,
                durationMs = o.optLong("duration") * 1000,
                coverUrl = o.optJSONObject("artwork")?.optString("480x480")?.takeIf(String::isNotBlank),
                source = Source.AUDIUS,
                playUrl = "https://api.audius.co/v1/tracks/$id/stream?app_name=musyc",
            )
        }
    }

    private fun fma(def: SourceDef): List<Song> {
        val html = getText(def.url.ifBlank { "https://freemusicarchive.org/music/charts/this-week" })
        val out = mutableListOf<Song>()
        var rest = html
        while (out.size < def.limit) {
            val start = rest.indexOf("data-track-info='")
            if (start < 0) break
            val after = rest.substring(start + "data-track-info='".length)
            val end = after.indexOf('\'')
            if (end < 0) break
            rest = after.substring(end + 1)
            val json = runCatching {
                JSONObject(Html.fromHtml(after.substring(0, end), Html.FROM_HTML_MODE_LEGACY).toString())
            }.getOrNull() ?: continue
            val id = json.optLong("id").takeIf { it > 0 } ?: continue
            val play = json.optString("fileUrl").takeIf(String::isNotBlank) ?: continue
            out += Song(
                id = id.toString(),
                title = json.optString("title", "未命名"),
                artists = json.optString("artistName"),
                album = json.optString("albumTitle"),
                coverUrl = json.optString("trackImage")
                    .ifBlank { json.optString("albumImage") }
                    .takeIf(String::isNotBlank),
                source = Source.CUSTOM,
                playUrl = play,
            )
        }
        if (out.isEmpty()) error("FMA 页面里没有找到可播放曲目")
        return out
    }

    private fun jamendo(def: SourceDef): List<Song> {
        val url = "https://api.jamendo.com/v3.0/playlists/tracks/?client_id=${def.clientId}" +
            "&id=${def.playlistId}&track_type=single+albumtrack&format=json&limit=${def.limit}"
        val json = getJson(url)
        val tracks = json.optJSONArray("results")?.optJSONObject(0)?.optJSONArray("tracks")
            ?: JSONArray()
        return (0 until tracks.length()).mapNotNull { i ->
            val o = tracks.optJSONObject(i) ?: return@mapNotNull null
            val play = o.optString("audio").ifBlank { o.optString("audiodownload") }
            if (play.isBlank()) return@mapNotNull null
            Song(
                id = o.optString("id"),
                title = o.optString("name", "未命名"),
                artists = o.optString("artist_name"),
                album = o.optString("album_name"),
                durationMs = o.optLong("duration") * 1000,
                coverUrl = o.optString("image").takeIf(String::isNotBlank),
                source = Source.CUSTOM,
                playUrl = play,
            )
        }
    }

    private fun generic(def: SourceDef): List<Song> {
        val root = getJson(def.url)
        val array = path(root, def.listPath) as? JSONArray
            ?: firstArray(root)
            ?: error("JSON 里没有找到数组")
        val out = mutableListOf<Song>()
        for (i in 0 until array.length()) {
            if (out.size >= def.limit) break
            val o = array.optJSONObject(i) ?: continue
            val id = string(o, def.idPath) ?: continue
            val play = string(o, def.playUrlPath) ?: continue
            val duration = durationMs(path(o, def.durationPath), def.durationUnit)
            out += Song(
                id = id,
                title = string(o, def.namePath) ?: "未命名",
                artists = artists(o, def.artistsPath),
                album = string(o, def.albumPath).orEmpty(),
                durationMs = duration,
                coverUrl = string(o, def.coverPath),
                source = Source.CUSTOM,
                playUrl = play,
                lyricUrl = string(o, def.lyricPath),
            )
        }
        return out
    }

    private fun artists(o: JSONObject, path: String): String {
        val v = this.path(o, path) ?: o.opt("artist") ?: o.opt("artists")
        return when (v) {
            is String -> v
            is JSONArray -> (0 until v.length()).joinToString(" / ") { i ->
                when (val x = v.opt(i)) {
                    is String -> x
                    is JSONObject -> x.optString("name")
                    else -> ""
                }
            }.trim().trim('/')
            is JSONObject -> v.optString("name")
            else -> ""
        }
    }

    private fun durationMs(v: Any?, unit: String): Long {
        val raw = when (v) {
            is Number -> v.toLong()
            is String -> v.toDoubleOrNull()?.toLong() ?: 0
            else -> 0
        }
        return if (unit.equals("s", true) || unit.equals("second", true)) raw * 1000 else raw
    }

    private fun string(o: JSONObject, path: String): String? =
        path(o, path)?.let {
            when (it) {
                is String -> it
                is Number -> it.toString()
                else -> null
            }
        }?.takeIf(String::isNotBlank)

    private fun path(root: Any?, path: String): Any? {
        if (path.isBlank()) return root
        var cur: Any? = root
        path.split('.').forEach { raw ->
            val name = raw.substringBefore('[')
            val index = raw.substringAfter('[', "").substringBefore(']').toIntOrNull()
            if (name.isNotBlank()) {
                cur = when (cur) {
                    is JSONObject -> cur.opt(name)
                    is JSONArray -> cur.opt(name.toIntOrNull() ?: -1)
                    else -> null
                }
            }
            if (index != null) {
                cur = (cur as? JSONArray)?.opt(index)
            }
        }
        return cur
    }

    private fun firstArray(root: JSONObject): JSONArray? {
        listOf("songs", "result", "results", "data", "tracks", "items").forEach { key ->
            (root.opt(key) as? JSONArray)?.let { return it }
        }
        return null
    }

    private fun getJson(url: String): JSONObject =
        JSONObject(getText(url))

    private fun getText(url: String): String {
        val resp = http.newCall(Request.Builder().url(url).header("User-Agent", UA).build()).execute()
        resp.use {
            if (!it.isSuccessful) error("请求失败 HTTP ${it.code}")
            return it.body?.string() ?: error("响应为空")
        }
    }
}

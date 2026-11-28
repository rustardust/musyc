package dev.musyc.app.data

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/** 最近播放，存 SharedPreferences，最多 50 首。 */
class RecentStore(context: Context) {
    private val prefs = context.getSharedPreferences("recent", Context.MODE_PRIVATE)

    fun load(): List<Song> {
        val raw = prefs.getString(KEY, null) ?: return emptyList()
        return runCatching {
            val arr = JSONArray(raw)
            (0 until arr.length()).mapNotNull { i ->
                val o = arr.optJSONObject(i) ?: return@mapNotNull null
                val source = runCatching { Source.valueOf(o.getString("source")) }.getOrNull()
                    ?: return@mapNotNull null
                Song(
                    id = o.getString("id"),
                    title = o.getString("title"),
                    artists = o.optString("artists"),
                    album = o.optString("album"),
                    durationMs = o.optLong("duration"),
                    coverUrl = o.optString("cover").takeIf(String::isNotBlank),
                    source = source,
                    playUrl = o.optString("play").takeIf(String::isNotBlank),
                    lyricUrl = o.optString("lyric").takeIf(String::isNotBlank),
                    localUri = o.optString("uri").takeIf(String::isNotBlank),
                )
            }
        }.getOrDefault(emptyList())
    }

    fun push(song: Song) {
        val list = load().filterNot { it.key == song.key }.toMutableList()
        list.add(0, song)
        write(list)
    }

    fun remove(song: Song) {
        write(load().filterNot { it.key == song.key })
    }

    private fun write(list: List<Song>) {
        val arr = JSONArray()
        list.take(50).forEach { s ->
            arr.put(
                JSONObject()
                    .put("id", s.id)
                    .put("title", s.title)
                    .put("artists", s.artists)
                    .put("album", s.album)
                    .put("duration", s.durationMs)
                    .put("cover", s.coverUrl ?: "")
                    .put("source", s.source.name)
                    .put("play", s.playUrl ?: "")
                    .put("lyric", s.lyricUrl ?: "")
                    .put("uri", s.localUri ?: "")
            )
        }
        prefs.edit().putString(KEY, arr.toString()).apply()
    }

    private companion object {
        const val KEY = "songs"
    }
}

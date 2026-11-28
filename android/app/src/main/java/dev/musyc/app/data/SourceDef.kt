package dev.musyc.app.data

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/** 首页三个在线槽位可绑定的歌单类型。 */
enum class SourceType(val id: String) {
    NETEASE_CHART("netease-chart"),
    NETEASE_PLAYLIST("netease-playlist"),
    STREETVOICE_RECOMMEND("streetvoice"),
    STREETVOICE_CAT("streetvoice-cat"),
    INDIE("indie"),
    AUDIUS("audius-trending"),
    FMA("fma-charts"),
    JAMENDO("jamendo-playlist"),
    GENERIC("generic-json"),
    ;

    companion object {
        fun parse(raw: String): SourceType? =
            entries.firstOrNull { it.id == raw.trim().lowercase() }
    }
}

/**
 * 一个歌单源，对应桌面端 sources 目录里的 .toml 的一项。
 * 内置项来自 [SourceCatalog]，自定义项可通过「导入 .toml」加入。
 */
data class SourceDef(
    val id: String,
    val name: String,
    val desc: String = "",
    val type: SourceType,
    val playlistId: String = "",
    val cat: String = "",
    val url: String = "",
    val clientId: String = "3dce8b55",
    val limit: Int = 50,
    val time: String = "week",
    val listPath: String = "",
    val idPath: String = "id",
    val namePath: String = "title",
    val artistsPath: String = "artist",
    val albumPath: String = "album",
    val durationPath: String = "duration",
    val durationUnit: String = "s",
    val coverPath: String = "cover",
    val playUrlPath: String = "url",
    val lyricPath: String = "",
    val custom: Boolean = false,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("id", id)
        .put("name", name)
        .put("desc", desc)
        .put("type", type.id)
        .put("playlist", playlistId)
        .put("cat", cat)
        .put("url", url)
        .put("client", clientId)
        .put("limit", limit)
        .put("time", time)
        .put("listPath", listPath)
        .put("idPath", idPath)
        .put("namePath", namePath)
        .put("artistsPath", artistsPath)
        .put("albumPath", albumPath)
        .put("durationPath", durationPath)
        .put("durationUnit", durationUnit)
        .put("coverPath", coverPath)
        .put("playUrlPath", playUrlPath)
        .put("lyricPath", lyricPath)

    companion object {
        fun fromJson(o: JSONObject): SourceDef? {
            val type = SourceType.parse(o.optString("type")) ?: return null
            return SourceDef(
                id = o.optString("id").ifBlank { return null },
                name = o.optString("name").ifBlank { o.optString("id") },
                desc = o.optString("desc"),
                type = type,
                playlistId = o.optString("playlist"),
                cat = o.optString("cat"),
                url = o.optString("url"),
                clientId = o.optString("client", "3dce8b55"),
                limit = o.optInt("limit", 50),
                time = o.optString("time", "week"),
                listPath = o.optString("listPath"),
                idPath = o.optString("idPath", "id"),
                namePath = o.optString("namePath", "title"),
                artistsPath = o.optString("artistsPath", "artist"),
                albumPath = o.optString("albumPath", "album"),
                durationPath = o.optString("durationPath", "duration"),
                durationUnit = o.optString("durationUnit", "s"),
                coverPath = o.optString("coverPath", "cover"),
                playUrlPath = o.optString("playUrlPath", "url"),
                lyricPath = o.optString("lyricPath"),
                custom = true,
            )
        }
    }
}

object SourceCatalog {
    val builtin: List<SourceDef> = listOf(
        SourceDef(
            id = "netease-chart",
            name = "网易原创",
            desc = "网易原创榜 · 官方直链",
            type = SourceType.NETEASE_CHART,
        ),
        SourceDef(
            id = "streetvoice",
            name = "街声推荐",
            desc = "街声 · HLS",
            type = SourceType.STREETVOICE_RECOMMEND,
        ),
        SourceDef(
            id = "indie",
            name = "雀乐推荐",
            desc = "雀乐 · 每期精选",
            type = SourceType.INDIE,
        ),
        SourceDef(
            id = "audius-week",
            name = "Audius 本周",
            desc = "Audius 热门榜 · 免费 Open API",
            type = SourceType.AUDIUS,
            time = "week",
            limit = 50,
        ),
        SourceDef(
            id = "fma-this-week",
            name = "FMA 本周排行",
            desc = "Free Music Archive · This Week",
            type = SourceType.FMA,
            url = "https://freemusicarchive.org/music/charts/this-week",
        ),
        SourceDef(
            id = "jamendo-singer-songwriter",
            name = "Jamendo 创作歌手",
            desc = "Singer-Songwriter · CC 协议",
            type = SourceType.JAMENDO,
            playlistId = "500608899",
        ),
        SourceDef(
            id = "streetvoice-alternative",
            name = "街声 Alternative",
            desc = "街声 Alternative 分类 · 最新",
            type = SourceType.STREETVOICE_CAT,
            cat = "7",
        ),
    )

    fun all(custom: List<SourceDef>): List<SourceDef> = builtin + custom

    fun byId(id: String, custom: List<SourceDef>): SourceDef? = all(custom).firstOrNull { it.id == id }
}

/**
 * 槽位绑定与自定义源持久化。
 * 兼容桌面端 TOML 的字段名，只做逐行 key = "value" 的宽松解析。
 */
class SourceStore(context: Context) {
    private val prefs = context.getSharedPreferences("sources", Context.MODE_PRIVATE)

    fun slotIds(): List<String> {
        val raw = prefs.getString(KEY_SLOTS, null) ?: return DEFAULT_SLOTS
        val arr = runCatching { JSONArray(raw) }.getOrNull() ?: return DEFAULT_SLOTS
        val ids = (0 until arr.length())
            .mapNotNull { arr.optString(it).takeIf(String::isNotBlank) }
            .filter { it != "indie" }   // 雀乐是固定槽位，不能绑到在线槽
        val filled = (ids + DEFAULT_SLOTS).distinct().take(3)
        return filled.ifEmpty { DEFAULT_SLOTS }
    }

    fun saveSlots(ids: List<String>) {
        prefs.edit().putString(KEY_SLOTS, JSONArray(ids.take(3)).toString()).apply()
    }

    fun custom(): List<SourceDef> {
        val raw = prefs.getString(KEY_CUSTOM, null) ?: return emptyList()
        val arr = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
        return (0 until arr.length()).mapNotNull { i ->
            arr.optJSONObject(i)?.let(SourceDef::fromJson)
        }
    }

    fun addCustom(def: SourceDef): Boolean {
        val list = custom().filterNot { it.id == def.id } + def
        val arr = JSONArray()
        list.forEach { arr.put(it.toJson()) }
        prefs.edit().putString(KEY_CUSTOM, arr.toString()).apply()
        return true
    }

    fun removeCustom(id: String) {
        val list = custom().filterNot { it.id == id }
        val arr = JSONArray()
        list.forEach { arr.put(it.toJson()) }
        prefs.edit().putString(KEY_CUSTOM, arr.toString()).apply()
    }

    companion object {
        private const val KEY_SLOTS = "slots"
        private const val KEY_CUSTOM = "custom"
        private val DEFAULT_SLOTS = listOf("netease-chart", "streetvoice", "audius-week")

        /** 解析桌面端 sources 目录里的 .toml；字段缺失时返回 null 由调用方提示。 */
        fun parseToml(text: String): SourceDef? {
            val values = HashMap<String, String>()
            text.lineSequence().forEach { line ->
                val trimmed = line.trim()
                if (trimmed.isEmpty() || trimmed.startsWith("#") || trimmed.startsWith("[")) return@forEach
                val eq = trimmed.indexOf('=')
                if (eq <= 0) return@forEach
                val key = trimmed.substring(0, eq).trim()
                var value = trimmed.substring(eq + 1).trim()
                if (value.startsWith("\"") && value.endsWith("\"") && value.length >= 2) {
                    value = value.substring(1, value.length - 1)
                }
                values[key] = value
            }
            val id = values["id"].orEmpty()
            val kind = values["kind"].orEmpty()
            val type = SourceType.parse(kind) ?: return null
            if (id.isBlank()) return null
            return SourceDef(
                id = id,
                name = values["name"].orEmpty().ifBlank { id },
                desc = values["desc"].orEmpty(),
                type = type,
                playlistId = values["playlist_id"].orEmpty(),
                cat = values["cat"].orEmpty(),
                url = values["url"].orEmpty(),
                clientId = values["client_id"].orEmpty().ifBlank { "3dce8b55" },
                limit = values["limit"]?.toIntOrNull() ?: 50,
                time = values["time"].orEmpty().ifBlank { "week" },
                listPath = values["list_path"].orEmpty(),
                idPath = values["id_path"].orEmpty().ifBlank { "id" },
                namePath = values["name_path"].orEmpty().ifBlank { "title" },
                artistsPath = values["artists_path"].orEmpty().ifBlank { "artist" },
                albumPath = values["album_path"].orEmpty().ifBlank { "album" },
                durationPath = values["duration_path"].orEmpty().ifBlank { "duration" },
                durationUnit = values["duration_unit"].orEmpty().ifBlank { "s" },
                coverPath = values["cover_path"].orEmpty().ifBlank { "cover" },
                playUrlPath = values["play_url_path"].orEmpty().ifBlank { "url" },
                lyricPath = values["lyric_url_path"].orEmpty(),
                custom = true,
            )
        }
    }
}

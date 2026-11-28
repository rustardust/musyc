package dev.musyc.app.data

import android.util.Base64
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.FormBody
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import java.math.BigInteger
import java.security.SecureRandom
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.spec.IvParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * 网易云音乐 Web API（weapi）直连实现。
 *
 * 与桌面端一致：只用官方直链与官方 30 秒试听，不做任何解灰。
 * 不登录，因此会员曲目只会拿到试听片段或空地址。
 */
object NeteaseApi {
    private const val BASE = "https://music.163.com"
    private const val UA =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
            "(KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"
    private const val PRESET_KEY = "0CoJUm6Qyw8W8jud"
    private const val IV = "0102030405060708"
    private const val RSA_N =
        "00e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a5aa76d2e4176" +
            "29ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685b7a46bee255932575cce" +
            "10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7"
    private const val RSA_E = "010001"
    private const val CHART_ID = "2884035"

    private val http: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(20, TimeUnit.SECONDS)
        .build()

    private val random = SecureRandom()

    suspend fun chart(limit: Int = 100): List<Song> = withContext(Dispatchers.IO) {
        val detail = post("/weapi/v6/playlist/detail", JSONObject().put("id", CHART_ID).put("n", "1000").put("s", "0"))
        val tracks = detail.optJSONObject("playlist")?.optJSONArray("tracks") ?: JSONArray()
        val songs = mutableListOf<Song>()
        for (i in 0 until tracks.length()) {
            songFrom(tracks.optJSONObject(i))?.let { songs.add(it) }
            if (songs.size >= limit) break
        }
        if (songs.isNotEmpty()) return@withContext songs
        // 部分情况下 detail 只回 trackIds，再用 song/detail 补全。
        val ids = detail.optJSONObject("playlist")?.optJSONArray("trackIds") ?: return@withContext emptyList()
        val idList = (0 until ids.length()).mapNotNull { ids.optJSONObject(it)?.optString("id") }.take(limit)
        songDetail(idList)
    }

    suspend fun searchSongs(keyword: String, limit: Int = 30): List<Song> = withContext(Dispatchers.IO) {
        // 公开 GET 接口：weapi 版本在无有效 csrf 时返回 50000005，这里直接走 pc 接口。
        val url = BASE + "/api/cloudsearch/pc?s=" + java.net.URLEncoder.encode(keyword, "UTF-8") +
            "&type=1&limit=" + limit + "&offset=0"
        val json = get(url)
        val songs = json.optJSONObject("result")?.optJSONArray("songs") ?: JSONArray()
        (0 until songs.length()).mapNotNull { songFrom(songs.optJSONObject(it)) }
    }

    suspend fun songDetail(ids: List<String>): List<Song> = withContext(Dispatchers.IO) {
        if (ids.isEmpty()) return@withContext emptyList()
        val out = mutableListOf<Song>()
        ids.chunked(100).forEach { chunk ->
            val data = JSONObject().put("c", "[" + chunk.joinToString(",") { "\"$it\"" } + "]")
            val json = post("/weapi/v3/song/detail", data)
            val songs = json.optJSONArray("songs") ?: JSONArray()
            for (i in 0 until songs.length()) {
                songFrom(songs.optJSONObject(i))?.let(out::add)
            }
        }
        out
    }

    /** 返回播放地址；官方直链拿不到时返回 null（调用方跳过该曲）。 */
    suspend fun streamUrl(id: String, level: String = "exhigh"): String? = withContext(Dispatchers.IO) {
        val data = JSONObject()
            .put("ids", "[$id]")
            .put("level", level)
            .put("encodeType", "flac")
            .put("csrf_token", "")
        val first = post("/weapi/song/enhance/player/url/v1", data)
            .optJSONArray("data")?.optJSONObject(0)
        val url = first?.optString("url").orEmpty()
        url.ifBlank { null }
    }

    suspend fun lyric(id: String): Lyric = withContext(Dispatchers.IO) {
        val data = JSONObject()
            .put("id", id)
            .put("lv", -1)
            .put("kv", -1)
            .put("tv", -1)
            .put("csrf_token", "")
        val json = runCatching { post("/weapi/song/lyric", data) }.getOrNull()
            ?: return@withContext Lyric(emptyList())
        val lrc = json.optJSONObject("lrc")?.optString("lyric").orEmpty()
        val trans = json.optJSONObject("tlyric")?.optString("lyric").orEmpty()
        LyricParser.parse(lrc, trans)
    }

    private fun songFrom(o: JSONObject?): Song? {
        if (o == null) return null
        val id = o.optString("id").ifBlank { return null }
        val name = o.optString("name").ifBlank { return null }
        val artists = o.optJSONArray("ar")?.let { arr ->
            (0 until arr.length()).mapNotNull { arr.optJSONObject(it)?.optString("name")?.takeIf(String::isNotBlank) }
        }.orEmpty()
        val album = o.optJSONObject("al")
        val cover = album?.optString("picUrl")?.takeIf { it.startsWith("http") }
        return Song(
            id = id,
            title = name,
            artists = artists.joinToString(" / "),
            album = album?.optString("name").orEmpty(),
            durationMs = o.optLong("dt"),
            coverUrl = cover,
            source = Source.NETEASE,
        )
    }

    private fun post(path: String, data: JSONObject): JSONObject {
        val secret = randomKey()
        val params = aes(aes(data.toString(), PRESET_KEY), secret)
        val encSecKey = rsa(secret.reversed())
        val body = FormBody.Builder()
            .add("params", params)
            .add("encSecKey", encSecKey)
            .build()
        val request = Request.Builder()
            .url(BASE + path)
            .header("User-Agent", UA)
            .header("Referer", "https://music.163.com/")
            .header("Cookie", "os=pc; appver=2.9.7; osver=10; channel=netease")
            .post(body)
            .build()
        http.newCall(request).execute().use { resp ->
            val text = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) error("网易云 HTTP ${resp.code}")
            return JSONObject(text)
        }
    }

    private fun get(url: String): JSONObject {
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", UA)
            .header("Referer", "https://music.163.com/")
            .header("Cookie", "os=pc; appver=2.9.7; osver=10; channel=netease")
            .get()
            .build()
        http.newCall(request).execute().use { resp ->
            val text = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) error("网易云 HTTP ${resp.code}")
            return JSONObject(text)
        }
    }

    private fun randomKey(): String {
        val chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        return (1..16).map { chars[random.nextInt(chars.length)] }.joinToString("")
    }

    private fun aes(text: String, key: String): String {
        val cipher = Cipher.getInstance("AES/CBC/PKCS5Padding")
        cipher.init(
            Cipher.ENCRYPT_MODE,
            SecretKeySpec(key.toByteArray(), "AES"),
            IvParameterSpec(IV.toByteArray()),
        )
        return Base64.encodeToString(cipher.doFinal(text.toByteArray()), Base64.NO_WRAP)
    }

    private fun rsa(text: String): String {
        val hex = text.toByteArray().joinToString("") { "%02x".format(it) }
        val value = BigInteger(hex, 16)
        val result = value.modPow(BigInteger(RSA_E, 16), BigInteger(RSA_N, 16))
        return result.toString(16).padStart(256, '0')
    }
}

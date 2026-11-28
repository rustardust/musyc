package dev.musyc.app.data

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.concurrent.TimeUnit

/** 本地标签补全：网易匹配元数据 → 生成带标签/封面/歌词的副本，原文件不动。 */
object Tagger {
    data class Meta(
        val title: String,
        val artist: String,
        val album: String,
        val cover: ByteArray?,
        val coverMime: String,
        val lyrics: String,
    )

    private val http = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build()

    suspend fun complete(
        context: Context,
        song: Song,
        onStep: (String) -> Unit = {},
    ): String = withContext(Dispatchers.IO) {
        val uri = Uri.parse(song.localUri ?: error("本地文件不存在"))
        val info = fileInfo(context, uri)
        if (!info.name.endsWith(".mp3", true) && info.mime != "audio/mpeg") {
            error("仅支持 MP3 标签写入")
        }
        onStep("匹配中")
        val meta = lookup(song)
        val original = context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            ?: error("读取本地文件失败")
        val tagged = Id3Writer.write(
            audio = original,
            title = meta.title,
            artist = meta.artist,
            album = meta.album,
            cover = meta.cover,
            coverMime = meta.coverMime,
            lyrics = meta.lyrics,
        )
        onStep("写入中")
        saveCopy(context, meta, tagged)
    }

    /** 给下载得到的 MP3 字节写入标签；非 MP3 原样返回。 */
    suspend fun enrich(context: Context, song: Song, bytes: ByteArray, ext: String): ByteArray =
        withContext(Dispatchers.IO) {
            if (!ext.equals("mp3", true)) return@withContext bytes
            runCatching {
                val meta = lookup(song)
                Id3Writer.write(bytes, meta.title, meta.artist, meta.album, meta.cover, meta.coverMime, meta.lyrics)
            }.getOrDefault(bytes)
        }

    suspend fun lookup(song: Song): Meta = withContext(Dispatchers.IO) {
        val query = listOf(song.title, song.artists)
            .filter { it.isNotBlank() }
            .joinToString(" ")
        val candidates = runCatching { NeteaseApi.searchSongs(query, 10) }.getOrDefault(emptyList())
        val best = pick(song, candidates)
        val cover = best?.coverUrl?.let { download(it) }
        val lyric = if (best != null) {
            runCatching { NeteaseApi.lyric(best.id) }.getOrNull()?.let { l ->
                l.plain.ifBlank { l.lines.joinToString("\n") { it.text } }
            }.orEmpty()
        } else {
            ""
        }
        Meta(
            title = best?.title?.takeIf { it.isNotBlank() } ?: song.title,
            artist = best?.artists?.takeIf { it.isNotBlank() } ?: song.artists,
            album = best?.album.orEmpty(),
            cover = cover,
            coverMime = "image/jpeg",
            lyrics = lyric,
        )
    }

    private fun pick(song: Song, candidates: List<Song>): Song? {
        if (candidates.isEmpty()) return null
        fun norm(s: String) = s.lowercase().replace(Regex("[\\s　·・\\-—_()（）\\[\\]]"), "")
        val title = norm(song.title)
        val artist = norm(song.artists)
        return candidates.maxByOrNull { c ->
            var score = 0
            val ct = norm(c.title)
            if (ct == title) score += 4
            else if (ct.contains(title) || title.contains(ct)) score += 2
            if (artist.isNotBlank() && norm(c.artists).contains(artist)) score += 2
            score
        }?.takeIf { candidate ->
            norm(candidate.title).contains(title) || title.contains(norm(candidate.title))
        } ?: candidates.first()
    }

    private fun download(url: String): ByteArray? = runCatching {
        http.newCall(Request.Builder().url(url).build()).execute().use { resp ->
            if (!resp.isSuccessful) return null
            val bytes = resp.body?.bytes() ?: return null
            if (bytes.size > 8 * 1024 * 1024) null else bytes
        }
    }.getOrNull()

    private data class FileInfo(val name: String, val mime: String)

    private fun fileInfo(context: Context, uri: Uri): FileInfo {
        val projection = arrayOf(MediaStore.Audio.Media.DISPLAY_NAME, MediaStore.Audio.Media.MIME_TYPE)
        context.contentResolver.query(uri, projection, null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                return FileInfo(
                    name = c.getString(0).orEmpty(),
                    mime = c.getString(1).orEmpty(),
                )
            }
        }
        return FileInfo("", "")
    }

    private fun saveCopy(context: Context, meta: Meta, bytes: ByteArray): String {
        val name = listOf(meta.artist, meta.title)
            .filter { it.isNotBlank() }
            .joinToString(" - ")
            .ifBlank { meta.title }
            .replace(Regex("[\\\\/:*?\"<>|\\n\\r\\t]"), "_")
            .take(120) + ".mp3"
        val resolver = context.contentResolver
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val values = ContentValues().apply {
                put(MediaStore.Audio.Media.DISPLAY_NAME, name)
                put(MediaStore.Audio.Media.MIME_TYPE, "audio/mpeg")
                put(MediaStore.Audio.Media.RELATIVE_PATH, "${Environment.DIRECTORY_MUSIC}/musyc")
                put(MediaStore.Audio.Media.IS_MUSIC, 1)
                put(MediaStore.Audio.Media.IS_PENDING, 1)
            }
            val uri = resolver.insert(MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, values)
                ?: error("无法创建输出文件")
            try {
                resolver.openOutputStream(uri)?.use { it.write(bytes) } ?: error("无法写入输出文件")
                values.clear()
                values.put(MediaStore.Audio.Media.IS_PENDING, 0)
                resolver.update(uri, values, null, null)
            } catch (e: Exception) {
                resolver.delete(uri, null, null)
                throw e
            }
            return "音乐/musyc/$name"
        }
        val dir = File(
            context.getExternalFilesDir(Environment.DIRECTORY_MUSIC) ?: context.filesDir,
            "musyc",
        ).apply { mkdirs() }
        val file = File(dir, name)
        file.outputStream().use { it.write(bytes) }
        return file.absolutePath
    }

    /** 供批量补全复用：判断一首本地歌曲是否已有完整标签。 */
    fun needsTag(song: Song): Boolean =
        song.title.isBlank() || song.artists.isBlank() || song.artists == "<unknown>" ||
            song.album.isBlank()

    @Suppress("unused")
    private fun ByteArrayOutputStream.toBytes(): ByteArray = toByteArray()
}

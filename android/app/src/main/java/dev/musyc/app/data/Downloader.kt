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
import java.io.File
import java.util.concurrent.TimeUnit

/** 下载直接音频文件到系统「下载/musyc」。街声是 HLS 流，暂不支持离线保存。 */
object Downloader {
    private val http = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build()

    suspend fun download(
        context: Context,
        song: Song,
        onProgress: (Float) -> Unit = {},
    ): String = withContext(Dispatchers.IO) {
        if (song.source == Source.STREETVOICE) {
            error("街声是 HLS 流，暂不支持下载")
        }
        val url = song.playUrl ?: error("没有可下载的地址")
        val request = Request.Builder().url(url).build()
        val ext = url.substringBefore('?').substringAfterLast('.', "mp3").let {
            if (it.length in 2..4) it else "mp3"
        }
        val name = fileName(song, ext)

        http.newCall(request).execute().use { resp ->
            if (!resp.isSuccessful) error("下载失败 HTTP ${resp.code}")
            val body = resp.body ?: error("响应为空")
            val total = body.contentLength()
            val raw = java.io.ByteArrayOutputStream()
            body.byteStream().use { input ->
                copyWithProgress(input, raw, total, onProgress)
            }
            val bytes = Tagger.enrich(context, song, raw.toByteArray(), ext)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val values = ContentValues().apply {
                    put(MediaStore.Downloads.DISPLAY_NAME, name)
                    put(MediaStore.Downloads.MIME_TYPE, mime(ext))
                    put(MediaStore.Downloads.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/musyc")
                    put(MediaStore.Downloads.IS_PENDING, 1)
                }
                val resolver = context.contentResolver
                val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                    ?: error("无法创建下载文件")
                try {
                    resolver.openOutputStream(uri)?.use { out ->
                        out.write(bytes)
                    } ?: error("无法写入下载文件")
                    values.clear()
                    values.put(MediaStore.Downloads.IS_PENDING, 0)
                    resolver.update(uri, values, null, null)
                } catch (e: Exception) {
                    resolver.delete(uri, null, null)
                    throw e
                }
                "下载/musyc/$name"
            } else {
                val dir = File(
                    context.getExternalFilesDir(Environment.DIRECTORY_MUSIC) ?: context.filesDir,
                    "download",
                )
                dir.mkdirs()
                val file = File(dir, name)
                file.outputStream().use { it.write(bytes) }
                file.absolutePath
            }
        }
    }

    private fun copyWithProgress(
        input: java.io.InputStream,
        out: java.io.OutputStream,
        total: Long,
        onProgress: (Float) -> Unit,
    ) {
        val buffer = ByteArray(64 * 1024)
        var read = 0L
        while (true) {
            val n = input.read(buffer)
            if (n < 0) break
            out.write(buffer, 0, n)
            read += n
            if (total > 0) onProgress((read.toFloat() / total).coerceIn(0f, 1f))
        }
        onProgress(1f)
    }

    private fun fileName(song: Song, ext: String): String {
        val base = listOf(song.artists, song.title)
            .filter { it.isNotBlank() }
            .joinToString(" - ")
            .ifBlank { song.title }
        val safe = base.replace(Regex("[\\\\/:*?\"<>|\\n\\r\\t]"), "_").take(120)
        return "$safe.$ext"
    }

    private fun mime(ext: String): String = when (ext.lowercase()) {
        "flac" -> "audio/flac"
        "m4a", "mp4" -> "audio/mp4"
        "ogg" -> "audio/ogg"
        "wav" -> "audio/wav"
        else -> "audio/mpeg"
    }

    @Suppress("unused")
    private fun uriOf(name: String): Uri = Uri.parse(name)
}

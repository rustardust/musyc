package dev.musyc.app.data

import java.io.ByteArrayOutputStream

/**
 * 最小可用的 ID3v2.3 写入器：标题 / 歌手 / 专辑 / 封面(APIC) / 歌词(USLT)。
 * 只处理 MP3；写入时丢弃旧的 ID3v2 与 ID3v1，音频帧原样保留。
 */
object Id3Writer {
    fun write(
        audio: ByteArray,
        title: String,
        artist: String,
        album: String,
        cover: ByteArray?,
        coverMime: String = "image/jpeg",
        lyrics: String? = null,
    ): ByteArray {
        val frames = ByteArrayOutputStream()
        frames.write(textFrame("TIT2", title))
        frames.write(textFrame("TPE1", artist))
        if (album.isNotBlank()) frames.write(textFrame("TALB", album))
        if (cover != null && cover.isNotEmpty()) {
            frames.write(apicFrame(cover, coverMime))
        }
        if (!lyrics.isNullOrBlank()) {
            frames.write(usltFrame(lyrics))
        }
        val frameBytes = frames.toByteArray()
        val header = ByteArrayOutputStream()
        header.write(byteArrayOf('I'.code.toByte(), 'D'.code.toByte(), '3'.code.toByte(), 3, 0, 0))
        header.write(syncSafe(frameBytes.size))
        header.write(frameBytes)

        val audioStart = audioStart(audio)
        val audioEnd = audioEnd(audio)
        val out = ByteArrayOutputStream(header.size() + (audioEnd - audioStart).coerceAtLeast(0))
        header.writeTo(out)
        out.write(audio, audioStart, (audioEnd - audioStart).coerceAtLeast(0))
        return out.toByteArray()
    }

    /** 跳过已有 ID3v2（含可选 footer）。 */
    private fun audioStart(data: ByteArray): Int {
        if (data.size < 10) return 0
        if (data[0] != 'I'.code.toByte() || data[1] != 'D'.code.toByte() || data[2] != '3'.code.toByte()) return 0
        val size = (data[6].toInt() and 0x7f shl 21) or
            (data[7].toInt() and 0x7f shl 14) or
            (data[8].toInt() and 0x7f shl 7) or
            (data[9].toInt() and 0x7f)
        val footer = if (data[5].toInt() and 0x10 != 0) 10 else 0
        return (10 + size + footer).coerceIn(0, data.size)
    }

    /** 去掉末尾 128 字节的 ID3v1。 */
    private fun audioEnd(data: ByteArray): Int {
        if (data.size >= 128) {
            val at = data.size - 128
            if (data[at] == 'T'.code.toByte() && data[at + 1] == 'A'.code.toByte() &&
                data[at + 2] == 'G'.code.toByte()
            ) {
                return at
            }
        }
        return data.size
    }

    private fun textFrame(id: String, value: String): ByteArray {
        val payload = ByteArrayOutputStream()
        payload.write(1) // UTF-16 with BOM
        payload.write(byteArrayOf(0xFF.toByte(), 0xFE.toByte()))
        payload.write(value.toByteArray(Charsets.UTF_16LE))
        payload.write(byteArrayOf(0, 0))
        return frame(id, payload.toByteArray())
    }

    private fun apicFrame(image: ByteArray, mime: String): ByteArray {
        val payload = ByteArrayOutputStream()
        payload.write(0) // ISO-8859-1
        payload.write(mime.toByteArray(Charsets.ISO_8859_1))
        payload.write(0)
        payload.write(3) // front cover
        payload.write(0) // empty description
        payload.write(image)
        return frame("APIC", payload.toByteArray())
    }

    private fun usltFrame(lyrics: String): ByteArray {
        val payload = ByteArrayOutputStream()
        payload.write(1)
        payload.write("eng".toByteArray(Charsets.ISO_8859_1))
        payload.write(byteArrayOf(0xFF.toByte(), 0xFE.toByte()))
        payload.write(byteArrayOf(0, 0)) // empty descriptor
        payload.write(byteArrayOf(0xFF.toByte(), 0xFE.toByte()))
        payload.write(lyrics.toByteArray(Charsets.UTF_16LE))
        return frame("USLT", payload.toByteArray())
    }

    private fun frame(id: String, payload: ByteArray): ByteArray {
        val out = ByteArrayOutputStream(10 + payload.size)
        out.write(id.toByteArray(Charsets.ISO_8859_1))
        out.write((payload.size ushr 24) and 0xff)
        out.write((payload.size ushr 16) and 0xff)
        out.write((payload.size ushr 8) and 0xff)
        out.write(payload.size and 0xff)
        out.write(0)
        out.write(0)
        out.write(payload)
        return out.toByteArray()
    }

    private fun syncSafe(size: Int): ByteArray = byteArrayOf(
        ((size ushr 21) and 0x7f).toByte(),
        ((size ushr 14) and 0x7f).toByte(),
        ((size ushr 7) and 0x7f).toByte(),
        (size and 0x7f).toByte(),
    )
}

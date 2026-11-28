package dev.musyc.app.data

import java.util.TreeMap

object LyricParser {
    private val timeTag = Regex("\\[(\\d{1,2}):(\\d{1,2})(?:[.:](\\d{1,3}))?]")

    fun parse(lrc: String, translation: String = ""): Lyric {
        val trans = parseSynced(translation)
        val map = TreeMap<Long, MutableList<String>>()
        var sawTag = false
        lrc.lineSequence().forEach { raw ->
            val tags = timeTag.findAll(raw).toList()
            if (tags.isEmpty()) return@forEach
            sawTag = true
            val text = raw.substring(tags.last().range.last + 1).trim()
            if (text.isEmpty()) return@forEach
            tags.forEach { tag ->
                val ms = toMs(tag.groupValues)
                val line = trans[ms]?.let { "$text  $it" } ?: text
                map.getOrPut(ms) { mutableListOf() }.add(line)
            }
        }
        if (!sawTag) {
            return Lyric(emptyList(), lrc.trim())
        }
        val lines = map.map { (at, texts) -> LyricLine(at, texts.joinToString(" ")) }
        return Lyric(lines)
    }

    private fun parseSynced(text: String): Map<Long, String> {
        val out = mutableMapOf<Long, String>()
        text.lineSequence().forEach { raw ->
            val tag = timeTag.find(raw) ?: return@forEach
            val content = raw.substring(tag.range.last + 1).trim()
            if (content.isNotEmpty()) out[toMs(tag.groupValues)] = content
        }
        return out
    }

    private fun toMs(groups: List<String>): Long {
        val min = groups.getOrNull(1)?.toLongOrNull() ?: 0
        val sec = groups.getOrNull(2)?.toLongOrNull() ?: 0
        val fracRaw = groups.getOrNull(3).orEmpty()
        val frac = when (fracRaw.length) {
            0 -> 0
            1 -> fracRaw.toLong() * 100
            2 -> fracRaw.toLong() * 10
            else -> fracRaw.take(3).toLong()
        }
        return min * 60_000 + sec * 1000 + frac
    }
}

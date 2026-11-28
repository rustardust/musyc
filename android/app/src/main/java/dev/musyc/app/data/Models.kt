package dev.musyc.app.data

enum class Source(val label: String) {
    NETEASE("网易云"),
    STREETVOICE("街声"),
    INDIE("雀乐"),
    AUDIUS("Audius"),
    CUSTOM("自定义"),
    LOCAL("本地"),
}

data class Song(
    val id: String,
    val title: String,
    val artists: String = "",
    val album: String = "",
    val durationMs: Long = 0,
    val coverUrl: String? = null,
    val source: Source,
    val playUrl: String? = null,
    val lyricUrl: String? = null,
    val localUri: String? = null,
) {
    val key: String get() = "${source.name.lowercase()}:$id"
}

data class Playlist(
    val id: String,
    val title: String,
    val subtitle: String = "",
    val coverUrl: String? = null,
    val songs: List<Song> = emptyList(),
)

data class LyricLine(val atMs: Long, val text: String)

data class Lyric(
    val lines: List<LyricLine>,
    val plain: String = "",
) {
    val synced: Boolean get() = lines.isNotEmpty()
}

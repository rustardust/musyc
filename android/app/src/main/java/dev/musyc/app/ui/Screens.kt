package dev.musyc.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.AutoFixHigh
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.musyc.app.data.Song

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SongsScreen(
    state: SongsUiState,
    playingKey: String?,
    canTag: Boolean = false,
    tagging: TagProgress? = null,
    onBack: () -> Unit,
    onPlay: (Int) -> Unit,
    onRefresh: () -> Unit = {},
    onTagAll: () -> Unit = {},
    onTagOne: ((Song) -> Unit)? = null,
    onDelete: ((Song) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    Column(modifier.fillMaxSize()) {
        TopAppBar(
            title = {
                Column {
                    Text(state.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (state.subtitle.isNotBlank()) {
                        Text(
                            state.subtitle,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            },
            navigationIcon = {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                }
            },
            actions = {
                if (canTag) {
                    IconButton(onClick = onTagAll, enabled = tagging == null) {
                        Icon(Icons.Default.AutoFixHigh, contentDescription = "补全标签")
                    }
                }
                IconButton(onClick = onRefresh) {
                    Icon(Icons.Default.Refresh, contentDescription = "刷新")
                }
            },
        )
        if (tagging != null) {
            LinearProgressIndicator(
                progress = { if (tagging.total > 0) tagging.done.toFloat() / tagging.total else 0f },
                modifier = Modifier.fillMaxWidth(),
            )
            Text(
                "补全中 ${tagging.done}/${tagging.total} · ${tagging.current}",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        when {
            state.loading && state.songs.isEmpty() -> Loading()
            state.error != null && state.songs.isEmpty() -> EmptyState(state.error)
            state.songs.isEmpty() -> EmptyState("这里还没有歌曲")
            else -> LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
                itemsIndexed(state.songs) { index, song ->
                    SongRow(song, index, playing = song.key == playingKey, onClick = { onPlay(index) }) {
                        if (onTagOne != null) {
                            IconButton(onClick = { onTagOne(song) }) {
                                Icon(
                                    Icons.Default.AutoFixHigh,
                                    contentDescription = "补全标签",
                                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                        if (onDelete != null) {
                            IconButton(onClick = { onDelete(song) }) {
                                Icon(
                                    Icons.Default.DeleteOutline,
                                    contentDescription = "移除",
                                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(
    state: SearchUiState,
    playingKey: String?,
    onBack: () -> Unit,
    onQuery: (String) -> Unit,
    onPlay: (Int) -> Unit,
    onOpenPlaylist: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    var text by remember { mutableStateOf(state.query) }
    Column(modifier.fillMaxSize()) {
        TopAppBar(
            title = { Text("搜索") },
            navigationIcon = {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                }
            },
        )
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            placeholder = { Text("搜索网易云歌曲") },
            leadingIcon = { Icon(Icons.Default.Search, contentDescription = null) },
            trailingIcon = {
                IconButton(onClick = { onQuery(text.trim()) }) {
                    Icon(Icons.Default.Search, contentDescription = "搜索")
                }
            },
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { onQuery(text.trim()) }),
        )
        Spacer(Modifier.padding(top = 8.dp))
        val empty = state.results.isEmpty() && state.playlists.isEmpty()
        when {
            state.loading && empty -> Loading()
            state.error != null && empty -> EmptyState(state.error)
            empty -> EmptyState("输入关键词，回车搜索")
            else -> LazyColumn(contentPadding = PaddingValues(top = 8.dp, bottom = 24.dp)) {
                if (state.results.isNotEmpty()) {
                    item(key = "songs-header") { SectionHeader("单曲") }
                    itemsIndexed(state.results) { index, song ->
                        SongRow(song, index, playing = song.key == playingKey, onClick = { onPlay(index) })
                    }
                }
                if (state.playlists.isNotEmpty()) {
                    item(key = "playlists-header") { SectionHeader("歌单") }
                    itemsIndexed(state.playlists) { index, playlist ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onOpenPlaylist(index) }
                                .padding(horizontal = 16.dp, vertical = 8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Cover(playlist.coverUrl, 56)
                            Spacer(Modifier.width(16.dp))
                            Column(Modifier.weight(1f)) {
                                Text(
                                    playlist.title,
                                    style = MaterialTheme.typography.titleSmall,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    playlist.subtitle,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun IndieScreen(
    state: IndieUiState,
    onBack: () -> Unit,
    onOpen: (Int) -> Unit,
    onRefresh: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    Column(modifier.fillMaxSize()) {
        TopAppBar(
            title = { Text("雀乐推荐") },
            navigationIcon = {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                }
            },
            actions = {
                IconButton(onClick = onRefresh) {
                    Icon(Icons.Default.Refresh, contentDescription = "刷新")
                }
            },
        )
        when {
            state.loading && state.playlists.isEmpty() -> Loading()
            state.error != null && state.playlists.isEmpty() -> EmptyState(state.error)
            state.playlists.isEmpty() -> EmptyState("暂无期数")
            else -> LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
                itemsIndexed(state.playlists) { index, playlist ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onOpen(index) }
                            .padding(horizontal = 16.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Cover(playlist.coverUrl, 56)
                        Spacer(Modifier.width(16.dp))
                        Column(Modifier.weight(1f)) {
                            Text(playlist.title, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(
                                playlist.subtitle,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LocalScreen(
    scanning: Boolean,
    onBack: () -> Unit,
    onScan: () -> Unit,
    onOpenFile: () -> Unit = {},
    onOpenM3u: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    Column(modifier.fillMaxSize()) {
        TopAppBar(
            title = { Text("本地音乐") },
            navigationIcon = {
                IconButton(onClick = onBack) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                }
            },
        )
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                if (scanning) {
                    CircularProgressIndicator()
                    Spacer(Modifier.padding(top = 12.dp))
                    Text("正在扫描…", color = MaterialTheme.colorScheme.onSurfaceVariant)
                } else {
                    Text(
                        "扫描手机里的音频文件（只读取媒体库）",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.padding(top = 16.dp))
                    Button(onClick = onScan) { Text("开始扫描") }
                    Spacer(Modifier.padding(top = 8.dp))
                    OutlinedButton(onClick = onOpenFile) { Text("打开本地文件") }
                    Spacer(Modifier.padding(top = 8.dp))
                    OutlinedButton(onClick = onOpenM3u) { Text("打开 M3U") }
                }
            }
        }
    }
}

@Composable
private fun Loading() {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(modifier = Modifier.size(32.dp), strokeWidth = 3.dp)
    }
}

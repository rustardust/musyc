package dev.musyc.app.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.PlaylistPlay
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Lyrics
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.QueueMusic
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.RepeatOne
import androidx.compose.material.icons.filled.Shuffle
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.Timer
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.musyc.app.data.Song
import dev.musyc.app.play.PlayMode
import dev.musyc.app.play.PlaybackUiState

/**
 * 底部播放条：承担全部播放控制，不再弹出单独的播放页。
 * 点击封面/标题：展开或收起全页歌词；右侧图标：模式 / 歌词 / 播放 / 下一首 / 更多。
 */
@Composable
fun MiniPlayer(
    state: PlaybackUiState,
    onOpen: () -> Unit,
    onMode: () -> Unit,
    onLyrics: () -> Unit,
    onToggle: () -> Unit,
    onNext: () -> Unit,
    onQueue: () -> Unit,
    onDownload: () -> Unit,
    onSleep: () -> Unit,
) {
    val song = state.song ?: return
    val progress = if (state.durationMs > 0) {
        (state.positionMs.toFloat() / state.durationMs).coerceIn(0f, 1f)
    } else {
        0f
    }
    Surface(tonalElevation = 3.dp, color = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.fillMaxWidth()) {
            MiniProgress(progress)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(start = 8.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Row(
                    modifier = Modifier
                        .weight(1f)
                        .clickable(onClick = onOpen),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Cover(song.coverUrl, 42)
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text(
                            song.title,
                            style = MaterialTheme.typography.bodyMedium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            song.artists.ifBlank { "未知歌手" },
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                IconButton(onClick = onMode) {
                    Icon(modeIcon(state.mode), contentDescription = "播放模式：${state.mode.label}")
                }
                IconButton(onClick = onLyrics) {
                    Icon(Icons.Default.Lyrics, contentDescription = "歌词")
                }
                IconButton(onClick = onToggle) {
                    Icon(
                        if (state.playing) Icons.Default.Pause else Icons.Default.PlayArrow,
                        contentDescription = "播放/暂停",
                    )
                }
                IconButton(onClick = onNext) {
                    Icon(Icons.Default.SkipNext, contentDescription = "下一首")
                }
                MoreMenu(onQueue = onQueue, onDownload = onDownload, onSleep = onSleep)
            }
        }
    }
}

@Composable
private fun MoreMenu(onQueue: () -> Unit, onDownload: () -> Unit, onSleep: () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.Default.MoreVert, contentDescription = "更多")
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(
                text = { Text("播放列表") },
                leadingIcon = { Icon(Icons.Default.QueueMusic, contentDescription = null) },
                onClick = {
                    open = false
                    onQueue()
                },
            )
            DropdownMenuItem(
                text = { Text("下载") },
                leadingIcon = { Icon(Icons.Default.Download, contentDescription = null) },
                onClick = {
                    open = false
                    onDownload()
                },
            )
            DropdownMenuItem(
                text = { Text("定时关闭") },
                leadingIcon = { Icon(Icons.Default.Timer, contentDescription = null) },
                onClick = {
                    open = false
                    onSleep()
                },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QueueSheet(
    queue: List<Song>,
    index: Int,
    onDismiss: () -> Unit,
    onPick: (Int) -> Unit,
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(Modifier.fillMaxWidth()) {
            Text(
                "播放列表 · ${queue.size} 首",
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
            )
            LazyColumn(modifier = Modifier.heightIn(max = 420.dp)) {
                itemsIndexed(queue) { i, song ->
                    SongRow(
                        song = song,
                        index = i,
                        playing = i == index,
                        onClick = { onPick(i) },
                    )
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
fun SleepDialog(onDismiss: () -> Unit, onPick: (Int) -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("定时关闭") },
        text = {
            Column {
                listOf(15, 30, 45, 60, 90).forEach { minutes ->
                    TextButton(onClick = { onPick(minutes) }, modifier = Modifier.fillMaxWidth()) {
                        Text("$minutes 分钟后")
                    }
                }
                TextButton(onClick = { onPick(0) }, modifier = Modifier.fillMaxWidth()) {
                    Text("取消定时")
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text("关闭") } },
    )
}

private fun modeIcon(mode: PlayMode) = when (mode) {
    PlayMode.Sequential -> Icons.AutoMirrored.Filled.PlaylistPlay
    PlayMode.ListLoop -> Icons.Default.Repeat
    PlayMode.Single -> Icons.Default.RepeatOne
    PlayMode.Shuffle -> Icons.Default.Shuffle
}

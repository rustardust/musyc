package dev.musyc.app.ui

import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.musyc.app.data.Lyric
import kotlinx.coroutines.delay
import kotlin.math.abs

/** 全页歌词：从屏幕左上角开始铺满，当前行居中高亮；点击播放条即可收起。 */
@Composable
fun LyricsScreen(
    lyric: Lyric?,
    loading: Boolean,
    positionMs: Long,
) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.statusBars),
    ) {
        when {
            loading -> CircularProgressIndicator(Modifier.align(Alignment.Center))
            lyric == null -> Empty(Modifier.align(Alignment.Center), "暂无歌词")
            lyric.lines.isEmpty() && lyric.plain.isBlank() ->
                Empty(Modifier.align(Alignment.Center), "暂无歌词")
            lyric.lines.isEmpty() -> PlainLyrics(lyric.plain)
            else -> SyncedLyrics(lyric, positionMs)
        }
    }
}

@Composable
private fun SyncedLyrics(lyric: Lyric, positionMs: Long) {
    val lines = lyric.lines
    val current = lines.indexOfLast { it.atMs <= positionMs }.coerceAtLeast(0)
    val state = rememberLazyListState()
    BoxWithConstraints(Modifier.fillMaxSize()) {
        // 先滚到当前行，再按实际布局把行中心对齐到视口中心（负偏移会因内容边界被裁剪）。
        LaunchedEffect(current) {
            state.animateScrollToItem(current)
            repeat(12) {
                val info = state.layoutInfo.visibleItemsInfo.firstOrNull { it.index == current }
                    ?: return@repeat
                val viewportCenter =
                    (state.layoutInfo.viewportStartOffset + state.layoutInfo.viewportEndOffset) / 2
                val delta = info.offset + info.size / 2 - viewportCenter
                if (abs(delta) <= 1) return@LaunchedEffect
                state.animateScrollBy(delta.toFloat())
                delay(16)
            }
        }
        LazyColumn(
            state = state,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(horizontal = 28.dp, vertical = maxHeight / 2),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            itemsIndexed(lines) { index, line ->
                Text(
                    text = line.text,
                    style = if (index == current) {
                        MaterialTheme.typography.titleMedium
                    } else {
                        MaterialTheme.typography.bodyLarge
                    },
                    color = if (index == current) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}

@Composable
private fun PlainLyrics(plain: String) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(start = 28.dp, end = 28.dp, top = 24.dp, bottom = 120.dp),
    ) {
        Text(
            text = plain,
            style = MaterialTheme.typography.bodyLarge,
            lineHeight = 32.sp,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun Empty(modifier: Modifier, text: String) {
    Text(
        text,
        modifier = modifier,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

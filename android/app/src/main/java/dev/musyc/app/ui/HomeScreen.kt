package dev.musyc.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.QueueMusic
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Storage
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    slots: List<HomeSlot>,
    onRefresh: () -> Unit,
    onSearch: () -> Unit,
    onOpenLink: () -> Unit,
    onSwap: (Int) -> Unit,
    onOpen: (HomeSlot) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier.fillMaxSize()) {
        TopAppBar(
            title = {
                Column {
                    Text("musyc", fontWeight = FontWeight.Bold)
                    Text(
                        "基于歌单的终端音乐盒",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            },
            actions = {
                IconButton(onClick = onOpenLink) {
                    Icon(Icons.Default.Link, contentDescription = "打开")
                }
                IconButton(onClick = onSearch) {
                    Icon(Icons.Default.Search, contentDescription = "搜索")
                }
                IconButton(onClick = onRefresh) {
                    Icon(Icons.Default.Refresh, contentDescription = "刷新")
                }
            },
        )
        LazyColumn(
            contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            itemsIndexed(slots) { index, slot ->
                SlotCard(
                    slot = slot,
                    onClick = { onOpen(slot) },
                    onSwap = if (slot.kind is SlotKind.Online) ({ onSwap(index) }) else null,
                )
            }
            item {
                Text(
                    "无登录 · 无解灰 · 歌单可插拔",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 16.dp),
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                )
            }
        }
    }
}

@Composable
private fun SlotCard(slot: HomeSlot, onClick: () -> Unit, onSwap: (() -> Unit)?) {
    val icon = when (slot.kind) {
        SlotKind.Local -> Icons.Default.Storage
        SlotKind.Recent -> Icons.Default.History
        else -> Icons.AutoMirrored.Filled.QueueMusic
    }
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
            .clickable(onClick = onClick),
        shape = RoundedCornerShape(20.dp),
        tonalElevation = 2.dp,
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (slot.coverUrl != null) {
                Cover(slot.coverUrl, 56)
            } else {
                Surface(
                    shape = RoundedCornerShape(14.dp),
                    color = MaterialTheme.colorScheme.primaryContainer,
                    modifier = Modifier.size(56.dp),
                ) {
                    Column(
                        modifier = Modifier.fillMaxSize(),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Icon(
                            icon,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onPrimaryContainer,
                        )
                    }
                }
            }
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f)) {
                Text(slot.title, style = MaterialTheme.typography.titleMedium)
                Text(
                    when {
                        slot.error != null -> slot.error
                        slot.loading -> "加载中…"
                        slot.kind is SlotKind.Indie -> "${slot.count} 期 · ${slot.subtitle}"
                        else -> if (slot.count > 0) "${slot.count} 首 · ${slot.subtitle}" else slot.subtitle
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = if (slot.error != null) MaterialTheme.colorScheme.error
                    else MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (slot.loading) {
                CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
            } else {
                if (onSwap != null) {
                    IconButton(onClick = onSwap) {
                        Icon(
                            Icons.Default.SwapHoriz,
                            contentDescription = "换源",
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                Icon(
                    Icons.Default.ChevronRight,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

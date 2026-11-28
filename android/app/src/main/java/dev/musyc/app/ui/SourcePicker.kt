package dev.musyc.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.musyc.app.data.SourceDef

/** 首页在线槽位的换源浮层，同时提供自定义 TOML 导入。 */
@Composable
fun SourcePickerDialog(
    sources: List<SourceDef>,
    currentId: String?,
    onPick: (SourceDef) -> Unit,
    onRemove: ((SourceDef) -> Unit)? = null,
    onImport: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("换源") },
        text = {
            LazyColumn(modifier = Modifier.heightIn(max = 420.dp)) {
                items(sources) { def ->
                    Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                        Column(
                            modifier = Modifier
                                .weight(1f)
                                .clickable { onPick(def) }
                                .padding(vertical = 10.dp),
                        ) {
                            Text(
                                text = def.name,
                                style = MaterialTheme.typography.titleSmall,
                                color = if (def.id == currentId) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.onSurface,
                            )
                            if (def.desc.isNotBlank()) {
                                Text(
                                    text = def.desc,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 2,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                        if (def.custom && onRemove != null) {
                            IconButton(onClick = { onRemove(def) }) {
                                Icon(
                                    Icons.Default.DeleteOutline,
                                    contentDescription = "删除自定义源",
                                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("关闭") }
        },
        dismissButton = {
            TextButton(onClick = onImport) { Text("导入 .toml") }
        },
    )
}

@Suppress("unused")
@Composable
private fun SelectedMark() {
    Icon(Icons.Default.Check, contentDescription = null)
}

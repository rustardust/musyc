use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use musyc_core::model::{PlayMode, Song};
use musyc_core::util::{format_duration, truncate_width};

use crate::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub fn render_player(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    current: Option<&Song>,
    playing: bool,
    loading: bool,
    pos_ms: u64,
    dur_ms: u64,
    mode: PlayMode,
    status: Option<String>,
) {
    // fox 底栏：无边框。上行整宽进度条，右端 `cur ● dur 模式`（像下划线，贴近底部）；
    // 下行居中 `歌名 歌手`（总宽 ≤ 可用宽度 60%，余留给通知，歌手不带样式）。
    if area.height == 0 || area.width == 0 {
        return;
    }
    let width = area.width as usize;

    // —— 上行：整宽进度条，右端 `cur ● dur 模式`（像下划线，贴近底部） ——
    let mode_text = mode.short_label().to_string();
    let note_icon = if loading {
        "…".to_string()
    } else if playing {
        "●".to_string()
    } else if current.is_some() {
        "■".to_string()
    } else {
        "·".to_string()
    };
    let time_text = if dur_ms > 0 {
        format!(
            "{} {} {}",
            format_duration(pos_ms),
            note_icon,
            format_duration(dur_ms)
        )
    } else {
        note_icon
    };
    let trailing = format!("{time_text} {mode_text}");
    let trailing_width = UnicodeWidthStr::width(trailing.as_str());
    let bar_width = width.saturating_sub(trailing_width + 1).max(1);
    let ratio = if dur_ms > 0 {
        (pos_ms.min(dur_ms) as f64 / dur_ms as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (bar_width as f64 * ratio).round() as usize;
    let rest = bar_width.saturating_sub(filled);
    let fill_style = if current.is_some() {
        Style::default().fg(theme.accent)
    } else {
        Style::default()
    };
    let bar = vec![
        Span::styled("━".repeat(filled), fill_style),
        Span::styled("┄".repeat(rest), Style::default()),
        Span::raw(" "),
        Span::raw(trailing),
    ];
    frame.render_widget(Paragraph::new(Line::from(bar)), Rect { height: 1, ..area });

    // —— 下行：居中 `歌名 歌手`（歌手不带样式，紧跟在歌名后面） ——
    if area.height >= 2 {
        let pad_cols: usize = 2; // 左右各预留 2 列
        let available = width.saturating_sub(2 * pad_cols); // 可用内容宽度
        let max_content = available * 6 / 10; // 60%宽度上限
        let artist = current.map(|song| song.artist_text()).unwrap_or_default();
        let artist_width = UnicodeWidthStr::width(artist.as_str());
        let raw_title = match current {
            Some(song) => song.name.clone(),
            None => "未选择歌曲".to_string(),
        };
        let title_width_raw = UnicodeWidthStr::width(raw_title.as_str());
        // 按实际文字长度分配：能放全就放全；超出 60% 时给短的一方完整显示，
        // 长的一方吃掉剩余预算；两边都长则各占一半（歌名不会比歌手短）。
        let budget = max_content
            .saturating_sub(if artist.is_empty() { 0 } else { 1 })
            .max(4);
        let (title_budget, artist_part) = if title_width_raw + artist_width <= budget {
            (title_width_raw, artist_width)
        } else if artist_width <= budget / 2 {
            (budget - artist_width, artist_width)
        } else if title_width_raw <= budget / 2 {
            (title_width_raw, budget - title_width_raw)
        } else {
            (budget / 2, budget - budget / 2)
        };
        let title_text = truncate_width(&raw_title, title_budget.max(1));
        let title_style = match current {
            Some(_) => Style::default()
                .fg(theme.bender)
                .add_modifier(Modifier::BOLD),
            None => Style::default(),
        };
        // 歌手不带样式，放在歌名之后；截断宽度与歌名预算计算一致（22 列）。
        let artist_style = Style::default();
        let artist_text = truncate_width(&artist, artist_part);
        let title_width = UnicodeWidthStr::width(title_text.as_str());
        let artist_width_disp = UnicodeWidthStr::width(artist_text.as_str());
        // 若总宽仍超可用宽度，再从歌名侧截断，保持歌手完整。
        let total_width = title_width
            .saturating_add(if artist_text.is_empty() { 0 } else { 1 })
            .saturating_add(artist_width_disp);
        let title_text = if total_width > max_content {
            let excess = total_width - max_content;
            truncate_width(&title_text, title_width.saturating_sub(excess).max(4))
        } else {
            title_text
        };
        // 重算 spans（标题可能已再截断）
        let mut spans = vec![Span::styled(title_text.clone(), title_style)];
        if !artist_text.is_empty() {
            spans.push(Span::styled(" ", Style::default()));
            spans.push(Span::styled(artist_text.clone(), artist_style));
        }
        let total_width: usize = spans.iter().map(|s| s.width()).sum();
        // 居中：左右各 pad_cols，剩余空间平分；右侧留给 hint 停靠
        let pad = pad_cols + (available.saturating_sub(total_width)) / 2;
        let mut line_spans: Vec<Span<'_>> = Vec::new();
        if let Some(hint) = status.as_deref()
            && pad >= 2
        {
            let clipped = truncate_width(hint, pad.saturating_sub(1));
            let clipped_width = UnicodeWidthStr::width(clipped.as_str());
            line_spans.push(Span::raw(clipped));
            line_spans.push(Span::raw(" ".repeat(pad.saturating_sub(clipped_width))));
        } else {
            line_spans.push(Span::raw(" ".repeat(pad)));
        }
        line_spans.extend(spans);
        frame.render_widget(
            Paragraph::new(Line::from(line_spans)),
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
        );
    }
}

/// 歌名加粗，` — 歌手` 保持普通样式；空间不足时先截断歌名，再截断歌手。
fn progress_text(label: &str, done: u64, total: Option<u64>) -> String {
    match total.filter(|t| *t > 0) {
        Some(total) => format!("{label} {}%", done * 100 / total),
        None => format!("{label}中…"),
    }
}

/// 底部状态行：提示优先，其次加载错误，最后标签补全 / 下载 / 缓存进度。
#[allow(clippy::too_many_arguments)]
/// 底部提示信息（toast / 错误 / 补全标签 / 下载 / 缓存）。
/// 无提示时返回 None；样式统一默认，由播放器第二行左侧停靠。
pub fn status_hint(
    toast: Option<&str>,
    error: Option<&str>,
    tag: Option<(u64, u64, &str)>,
    cache: Option<(u64, Option<u64>)>,
    download: Option<(u64, Option<u64>)>,
) -> Option<String> {
    if let Some(message) = toast {
        Some(message.to_string())
    } else if let Some(error) = error {
        // 把 R 重试放在最前面，窗口再窄也不会先被截掉。
        Some(format!("R 重试 · {error}"))
    } else if let Some((done, total, name)) = tag {
        if total > 0 {
            Some(format!("补全标签 {done}/{total} · {name}"))
        } else {
            Some(format!("补全标签 · {name}"))
        }
    } else if let Some((done, total)) = download {
        Some(progress_text("下载", done, total))
    } else if let Some((done, total)) = cache {
        Some(progress_text("缓存", done, total))
    } else {
        None
    }
}

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use musyc_core::util::truncate_width;

use crate::theme::Theme;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::types::{Focus, InputMode};

fn input_window(input: &str, cursor: usize, available: usize) -> (String, usize) {
    if available == 0 {
        return (String::new(), 0);
    }
    let cursor = cursor.min(input.len());
    let prefix_width = UnicodeWidthStr::width(&input[..cursor]);
    if prefix_width <= available {
        let mut out = String::new();
        let mut width = 0;
        for ch in input.chars() {
            let w = ch.width().unwrap_or(0);
            if width + w > available {
                break;
            }
            width += w;
            out.push(ch);
        }
        return (out, prefix_width.min(available));
    }

    let budget = available.saturating_sub(1).max(1);
    let mut start = cursor;
    let mut used = 0usize;
    for (idx, ch) in input[..cursor].char_indices().rev() {
        let w = ch.width().unwrap_or(0);
        if used + w > budget {
            break;
        }
        used += w;
        start = idx;
    }
    let mut out = String::from("…");
    out.push_str(&input[start..cursor]);
    let cursor_col = (1 + used).min(available);
    let mut width = 1 + used;
    for ch in input[cursor..].chars() {
        let w = ch.width().unwrap_or(0);
        if width + w > available {
            break;
        }
        width += w;
        out.push(ch);
    }
    (out, cursor_col)
}

#[allow(clippy::too_many_arguments)]
pub fn render_search(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    input: &str,
    cursor: usize,
    mode: InputMode,
    focus: Focus,
    loading: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // 单行搜索栏：只占 1 行，出现 / 消失时列表不再整体跳动 3 行。
    let mut label = mode.label().to_string();
    if loading {
        label.push_str(" · 加载中");
    }
    let accent_style = if focus == Focus::Input {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    let prefix = format!(" {label}  › ");
    let prefix_width = UnicodeWidthStr::width(prefix.as_str());
    let (prefix, prefix_width) = if prefix_width + 8 > area.width as usize {
        ("› ".to_string(), 2usize)
    } else {
        (prefix, prefix_width)
    };
    let available = (area.width as usize).saturating_sub(prefix_width);
    let (display, cursor_col) = input_window(input, cursor, available);
    let (shown, style) = if input.is_empty() {
        (
            truncate_width(mode.placeholder(), available),
            Style::default(),
        )
    } else {
        (display, Style::default())
    };

    let line = Line::from(vec![
        Span::styled(prefix, accent_style),
        Span::styled(shown, style),
    ]);
    frame.render_widget(Paragraph::new(line), area);

    if focus == Focus::Input {
        let x = area.x + (prefix_width + cursor_col).min(area.width as usize - 1) as u16;
        frame.set_cursor_position((x, area.y));
    }
}

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, TableState};

use musyc_core::util::{pad_width, truncate_width};

use crate::theme::Theme;

pub fn render_source_picker(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    items: &[(String, String, String)], // (name, desc, tag) tag = "内置"/"自定义"
    state: &mut TableState,
) {
    if area.width < 30 || area.height < 8 {
        return;
    }
    let max_width = area.width.saturating_sub(4).max(20);
    let width = ((area.width as u32 * 72 / 100) as u16)
        .clamp(20, max_width)
        .min(area.width);
    let max_height = area.height.saturating_sub(2).max(4);
    let height = ((items.len() as u16 + 3).min(max_height)).max(4);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    frame.render_widget(
        Clear,
        Rect {
            x: area.x,
            y: rect.y,
            width: area.width,
            height: rect.height,
        },
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::raw(format!(" {title} ")));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let inner_w = inner.width as usize;
    let lines = items
        .iter()
        .enumerate()
        .map(|(i, (name, desc, tag))| {
            let is_selected = state.selected() == Some(i);
            let style = if is_selected {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            // 序号 4 列 + 标签按宽度预留，名字/描述剩余宽度截断，整行不换行。
            let tag_w = tag.chars().count() + 4; // "  [tag]"
            let name_w = inner_w.saturating_sub(4 + tag_w).clamp(4, 24);
            let desc_w = inner_w.saturating_sub(4 + tag_w + name_w + 2);
            let name = pad_width(&truncate_width(name, name_w), name_w);
            let desc = truncate_width(desc, desc_w);
            Line::from(vec![
                Span::styled(format!("{:>2}. ", i + 1), Style::default().fg(theme.bender)),
                Span::styled(name, style),
                Span::styled(format!("  [{tag}]"), Style::default().dim()),
                if desc.is_empty() {
                    Span::raw(String::new())
                } else {
                    Span::styled(format!("  · {desc}"), Style::default().dim())
                },
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}

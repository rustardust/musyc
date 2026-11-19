use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, TableState};
use unicode_width::UnicodeWidthStr;

use musyc_core::util::{pad_width, truncate_width};

use crate::theme::Theme;
use crate::ui::list::draw_message;

/// 窗口低于最小尺寸时只显示这一行，避免各面板挤在一起。
pub fn render_too_small(frame: &mut Frame, area: Rect, _theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let message = truncate_width("终端太小，请放大窗口或按 l 隐藏歌词", area.width as usize);
    let line = Line::from(Span::raw(message)).alignment(Alignment::Center);
    frame.render_widget(
        Paragraph::new(line),
        Rect {
            y: area.y + (area.height / 2).saturating_sub(1),
            height: 2,
            ..area
        },
    );
}

/// fox 风格的圆角弹窗：macOS Toast 一样的边框 + 强调色标题。
/// 目前用于帮助浮层；歌词/列表主页不再使用边框。
/// 帮助浮层：圆角边框 + 默认样式标题，不再使用强调色。
pub fn block(title: &str, _theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::raw(format!(" {title} ")))
}

/// 首页字标：2 个终端行的「musyc」块字（M/U/S/Y/C 取自旧项目块字，去掉多余的 I），
/// 每字母 5 列等宽，由 render_home 居中显示，品牌色沿用 theme.bender。
const BRAND_ART: [&str; 2] = [
    "█▄ ▄█ █   █ ▄▀▀▀▀ ▀▄ ▄▀ ▄▀▀▀▀",
    "█ ▀ █ █▄▄▄█ ▄▄▄▄▀   █   ▀▄▄▄▄",
];

/// 首页入口列表。
pub fn render_home(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    items: &[&str],
    state: &mut TableState,
    _title: &str,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    if items.is_empty() {
        draw_message(frame, area, "栏目加载中…", Style::default());
        return;
    }

    let selected = state.selected();
    // 垂直居中：顶部一行空行 + 两行字标 + 一行空隙 + 单列菜单。
    let menu_h = items.len() as u16;
    let content_h = 1 + BRAND_ART.len() as u16 + 1 + menu_h;
    let top_pad = area.height.saturating_sub(content_h) / 2;

    let rows = Layout::vertical([
        Constraint::Length(top_pad),
        Constraint::Length(1), // 顶部空行
        Constraint::Length(BRAND_ART.len() as u16),
        Constraint::Length(1), // gap
        Constraint::Length(menu_h),
        Constraint::Min(0),
    ])
    .split(area);

    let art_lines = BRAND_ART.iter().map(|line| {
        Line::from(Span::styled(
            line.to_string(),
            Style::default()
                .fg(theme.bender)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
    });
    frame.render_widget(Paragraph::new(art_lines.collect::<Vec<_>>()), rows[2]);

    // 单列居中菜单：整块按最长一行宽度居中，文字左对齐；
    // 选中行整行使用暗色 pill 背景 + 金莲花橙粗体。
    let items_with_num: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, label)| format!("{}.  {}", i + 1, label))
        .collect();
    let text_w = items_with_num
        .iter()
        .map(|text| UnicodeWidthStr::width(text.as_str()))
        .max()
        .unwrap_or(0)
        .min(area.width as usize);
    let block_x = area.x + ((area.width as usize).saturating_sub(text_w) / 2) as u16;
    let block_area = Rect {
        x: block_x,
        y: rows[4].y,
        width: text_w as u16,
        height: menu_h.min(rows[4].height),
    };
    let menu_lines = items_with_num
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let is_selected = selected == Some(index);
            let style = if is_selected {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let padded = pad_width(&truncate_width(text, text_w).to_string(), text_w);
            Line::from(Span::styled(padded, style))
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(menu_lines), block_area);
}

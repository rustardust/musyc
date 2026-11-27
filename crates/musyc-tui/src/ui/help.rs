use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use musyc_core::util::{pad_width, truncate_width};

use crate::theme::Theme;
use crate::ui::block;

pub fn render_help(frame: &mut Frame, area: Rect, theme: &Theme, scroll: &mut u16) {
    if area.width < 20 || area.height < 5 {
        return;
    }
    let entries = [
        ("/", "搜索单曲（回车确认，Esc 取消）"),
        ("f", "搜索网易云歌单"),
        ("o", "打开路径 / 链接 / ID / M3U"),
        ("Enter", "打开栏目 / 播放选中歌曲 / 打开歌单"),
        ("Space", "播放 / 暂停"),
        ("] / [", "下一首 / 上一首"),
        ("← / →", "快退 / 快进 5 秒"),
        ("+ / -", "音量"),
        ("r", "切换播放模式"),
        ("l", "歌词：展开 → 纯享 → 关闭"),
        ("z", "定时关闭（输入分钟，留空取消）"),
        ("PgUp/PgDn", "滚动歌词"),
        ("t / T", "补全选中 / 缺失的本地标签并生成副本"),
        ("s", "换源（首页前三个在线入口，按选中项换绑）"),
        ("x / Del", "从最近播放删除"),
        ("d", "下载选中歌曲（写入标签/歌词/封面）"),
        ("g / G", "列表首 / 尾"),
        ("Esc", "返回上一级 / 回到首页"),
        ("R", "刷新当前栏目"),
        ("?", "快捷键"),
        ("q", "退出"),
    ];

    let max_width = area.width.saturating_sub(2).max(20);
    let width = ((area.width as u32 * 76 / 100) as u16)
        .clamp(20, max_width)
        .min(area.width);
    let max_height = area.height.saturating_sub(2).max(3);
    let height = ((area.height as u32 * 82 / 100) as u16)
        .min(entries.len() as u16 + 2)
        .min(max_height)
        .max(3);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    // 整行清空：box 边缘可能正好落在双宽字符的第二个单元格上，
    // 只清 box 内部会留下「半个」CJK 字形并吃掉边框单元格。
    // 整行置空则任何字符都被完整擦除，边框一定能正常显示。
    frame.render_widget(
        Clear,
        Rect {
            x: area.x,
            y: rect.y,
            width: area.width,
            height: rect.height,
        },
    );

    // 每项固定一行，超宽描述用省略号截断，滚动位置才和屏幕行一一对应。
    let inner_w = width.saturating_sub(2) as usize;
    let key_w = 12.min(inner_w);
    let desc_w = inner_w.saturating_sub(key_w);
    let lines = entries
        .iter()
        .map(|(key, desc)| {
            let key_text = pad_width(&truncate_width(&format!("{key:>8}  "), key_w), key_w);
            Line::from(vec![
                Span::styled(
                    key_text,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(truncate_width(desc, desc_w), Style::default()),
            ])
        })
        .collect::<Vec<_>>();

    let inner_h = height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(inner_h);
    let current = (*scroll as usize).min(max_scroll);
    *scroll = current as u16;
    let title = if max_scroll > 0 {
        "快捷键 · ↑/↓ 滚动"
    } else {
        "快捷键"
    };
    let paragraph = Paragraph::new(lines)
        .block(block(title, theme))
        .scroll((current as u16, 0));
    frame.render_widget(paragraph, rect);
}

use ratatui::prelude::*;

/// 横並びの選択肢を表示するwidget。
/// 左右キーでカーソル移動、Spaceでトグルする想定。
/// 画面幅に収まらない場合はカーソル位置に追従して横スクロールする。
pub struct HorizontalSelector<'a> {
    items: &'a [(&'a str, bool)],
    cursor: usize,
    /// 項目間のスペース数
    gap: usize,
}

impl<'a> HorizontalSelector<'a> {
    /// items: &[(label, selected)]
    pub fn new(items: &'a [(&'a str, bool)], cursor: usize) -> Self {
        Self {
            items,
            cursor,
            gap: 2,
        }
    }

    /// 各項目の(start, end)位置を計算。先頭1文字分のパディング込み。
    fn item_positions(&self) -> Vec<(usize, usize)> {
        let mut positions = Vec::with_capacity(self.items.len());
        let mut x = 1; // 先頭パディング
        for (i, (label, _)) in self.items.iter().enumerate() {
            if i > 0 {
                x += self.gap;
            }
            let end = x + label.len();
            positions.push((x, end));
            x = end;
        }
        positions
    }

    /// カーソル位置が見えるようにスクロールオフセットを計算
    fn scroll_offset(&self, width: usize) -> usize {
        let positions = self.item_positions();
        if self.cursor >= positions.len() {
            return 0;
        }
        let (_start, end) = positions[self.cursor];
        if end + 1 > width {
            // カーソル項目の右端が見えるようにスクロール（右端に1文字余裕）
            end + 1 - width
        } else {
            0
        }
    }
}

impl Widget for HorizontalSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 || self.items.is_empty() {
            return;
        }

        let width = area.width as usize;
        let offset = self.scroll_offset(width);
        let positions = self.item_positions();
        let gap_str = " ".repeat(self.gap);

        // スクロールインジケータ（左端）
        if offset > 0 {
            let style = Style::default().fg(Color::DarkGray);
            buf.set_string(area.x, area.y, "◀", style);
        }

        for (i, ((label, selected), &(start, end))) in
            self.items.iter().zip(positions.iter()).enumerate()
        {
            let is_cursor = i == self.cursor;

            // セパレータ描画
            if i > 0 {
                let sep_start = start - self.gap;
                let sep_end = start;
                self.render_slice(
                    buf,
                    area,
                    offset,
                    width,
                    sep_start,
                    sep_end,
                    &gap_str,
                    Style::default(),
                );
            }

            // 項目描画
            let style = item_style(*selected, is_cursor);
            self.render_slice(buf, area, offset, width, start, end, label, style);
        }

        // スクロールインジケータ（右端）
        let total_width = positions.last().map(|(_, e)| *e).unwrap_or(0);
        if total_width > offset + width {
            let style = Style::default().fg(Color::DarkGray);
            buf.set_string(area.x + area.width - 1, area.y, "▶", style);
        }
    }
}

impl HorizontalSelector<'_> {
    /// 文字列の見える部分だけをバッファに書き込む
    fn render_slice(
        &self,
        buf: &mut Buffer,
        area: Rect,
        offset: usize,
        width: usize,
        start: usize,
        end: usize,
        text: &str,
        style: Style,
    ) {
        // 表示範囲外なら何もしない
        if end <= offset || start >= offset + width {
            return;
        }

        let visible_start = if start < offset { offset - start } else { 0 };
        let visible_end = text.len().min(offset + width - start);
        if visible_start >= visible_end {
            return;
        }

        let screen_x = if start > offset {
            area.x + (start - offset) as u16
        } else {
            area.x
        };

        // スクロールインジケータと被らないようにする
        let min_x = if offset > 0 { area.x + 1 } else { area.x };
        let screen_x = screen_x.max(min_x);

        buf.set_string(screen_x, area.y, &text[visible_start..visible_end], style);
    }
}

fn item_style(selected: bool, is_cursor: bool) -> Style {
    if is_cursor && selected {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else if is_cursor {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else if selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

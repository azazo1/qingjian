//! 查询与呈现：刷新候选、画候选窗、整句补全、翻页与高亮。

use super::*;

impl QingjianInputController {
    /// 按当前缓冲区重新查候选、更新 marked text，回到第一页并重画候选窗口。
    pub(super) fn refresh(&self, client: TextClient<'_>) {
        let Some((marked, cursor, inline)) = host::with(|h| {
            // 查询失败 (整段切不动) 时退回显示原始字母; 还没交给应用的已选词排在前面
            let (mut marked, mut cursor) = h.engine.plain_preedit();
            let mut preedit = Preedit::plain(&marked, cursor);
            let candidates = h
                .engine
                .query()
                .map(|mut query| {
                    h.engine.annotate(&mut query.candidates);
                    marked = query.marked_text();
                    cursor = query.marked_cursor();
                    preedit =
                        Preedit::from_marked(&query.marked_segments(), query.segments_cursor());
                    query.candidates.items
                })
                .unwrap_or_default();
            h.reset_session(preedit, candidates);
            (marked, cursor, h.preedit_mode.inline())
        }) else {
            return;
        };
        // 配置成只在候选窗口显示拼音时, 应用里不放 marked text (光标位置仍按插入点取)
        if inline {
            client.set_marked_text(&marked, cursor);
        } else {
            client.set_marked_text("", 0);
        }
        self.render(client);
    }

    /// 缓冲区里只有一个 `?` 而用户按了别的键：把它还原成问号上屏（中文遵循标点设置、英文半角）、清空缓冲区。
    /// 返回是否发生了还原。
    pub(super) fn restore_bare_question(&self, client: TextClient<'_>) -> bool {
        let english = host::with(|h| h.refresh_mode()).unwrap_or(false);
        let restored = host::with(|h| {
            h.engine.restore_bare_question(english)
        })
        .flatten();
        let Some(mark) = restored else {
            return false;
        };
        client.insert_text(&mark);
        self.refresh(client);
        true
    }

    /// 记下光标位置并按会话状态重画候选窗口。
    pub(super) fn render(&self, client: TextClient<'_>) {
        let anchor = client.caret_rect();
        host::with(|h| {
            h.anchor = anchor;
            h.render();
        });
    }

    /// 上下键. 高亮逐个移动, 越过页边自动翻页; 横排矩阵开着 (`[general] horizontal_grid`) 时改为:
    /// 单行先展开成矩阵，在矩阵里换行（视口跟着滚）。
    pub(super) fn move_highlight(&self, delta: isize, client: TextClient<'_>) {
        let changed = host::with(|h| {
            if h.grid_keys() {
                h.session.move_rows(delta)
            } else {
                h.session.move_highlight(delta)
            }
        })
        .unwrap_or(false);
        if changed {
            self.render(client);
        }
    }

    /// 左右键。横排矩阵开着而且有候选时归候选：单行里逐个移动高亮（到页边自动翻页，不展开），展开后按阅读顺序跨行移动——
    /// 上下键被矩阵占了，单行里只剩左右键能挪高亮。返回是不是归了候选（不是的话调用方照旧移动拼音光标）。
    pub(super) fn move_cells(&self, delta: isize, client: TextClient<'_>) -> bool {
        let handled = host::with(|h| {
            if !h.grid_keys() || h.session.layout.is_empty() {
                return None;
            }
            Some(if h.session.grid.is_some() {
                h.session.move_cells(delta)
            } else {
                h.session.move_highlight(delta)
            })
        })
        .flatten();
        if handled == Some(true) {
            self.render(client);
        }
        handled.is_some()
    }

    /// 每个键都问一次应用标识（activateServer 时进程刚拉起可能还拿不到），变了才告诉 Engine。
    pub(super) fn note_application(&self, client: &TextClient<'_>) {
        let app = client.bundle_identifier();
        host::with(|h| {
            if h.engine.application() != app.as_deref() {
                h.engine.set_application(app);
            }
        });
    }

    /// 翻页，高亮落到新页第一项。已在首页 / 末页时不动。
    pub(super) fn turn_page(&self, delta: isize, client: TextClient<'_>) -> bool {
        let turned = host::with(|h| {
            // 矩阵展开着：翻页键一次翻一屏
            let turned = if h.session.grid.is_some() {
                h.session.move_screens(delta)
            } else {
                h.session.turn_page(delta)
            };
            if turned {
                h.engine.note_page_turn();
            }
            turned
        })
        .unwrap_or(false);
        if turned {
            self.render(client);
        }
        true
    }
}

//! 组句的展示状态：缓冲变化时重查候选并重建 [`Composed`]，云端词异步并入，高亮 / 翻页，按状态生成给 DLL 的帧。

mod state;

use qingjian_core::{Candidate, CandidateKind, CandidateLayout, CandidateList};
use qingjian_platform::protocol::{Frame, PreeditKind, PreeditSegment};

pub(super) use self::state::Composed;
use super::Router;

impl Router {
    /// 缓冲变化后：按 Engine 状态重建 [`Composed`]，发一次云联想请求，归零高亮与整句补全。
    pub(super) fn recompose(&mut self) {
        self.highlight = 0;
        self.navigated = false;
        self.sentence = None;
        if self.engine.composition().is_empty() {
            // 没在组句就没有候选可调，频次预览跟着归位
            self.preview_frequency = false;
            self.composed = None;
            self.stop_rescoring();
            return;
        }
        self.attach_loaded_model();
        let built = self.engine.query().ok().map(|query| {
            let items = query.candidates.items.clone();
            let preedit: Vec<PreeditSegment> =
                query.marked_segments().iter().map(Into::into).collect();
            // 光标用 Core 的映射：自动补的 `'` 会让显示串比敲的长。
            (items, preedit, query.marked_cursor())
        });
        self.composed = Some(match built {
            Some((items, preedit, cursor)) => {
                let layout =
                    CandidateLayout::new(items, self.config.page_size, self.config.cloud_slots);
                Composed::Candidates {
                    preedit,
                    cursor,
                    layout,
                }
            }
            None => {
                // 查询失败时退回显示原始字母；还没交给应用的已选词排在前面
                let (text, cursor) = self.engine.plain_preedit();
                Composed::Raw { text, cursor }
            }
        });
        self.highlight = (0..self.candidate_count())
            .find(|&index| self.layout_candidate(index).is_some())
            .unwrap_or(0);
        self.schedule_rescoring();
    }

    /// 高亮移动 `delta`，夹在 `[0, 末尾]`，到页边自然换页。
    pub(super) fn move_highlight(&mut self, delta: isize) {
        if delta == 0 {
            return;
        }
        let count = self.candidate_count();
        if count == 0 {
            self.highlight = 0;
            return;
        }
        let mut next = self.highlight as isize + delta.signum();
        while (0..count as isize).contains(&next) {
            if self.layout_candidate(next as usize).is_some() {
                self.navigated = true;
                self.highlight = next as usize;
                break;
            }
            next += delta.signum();
        }
    }

    /// 整页翻 `step`，高亮落到目标页第一个候选。
    pub(super) fn page(&mut self, step: isize) {
        if step == 0 {
            return;
        }
        let count = self.candidate_count();
        if count == 0 {
            self.highlight = 0;
            return;
        }
        let page_size = self.config.page_size;
        let page_count = count.div_ceil(page_size);
        let current = (self.highlight / page_size) as isize;
        let mut target = (current + step).clamp(0, page_count as isize - 1);
        while (0..page_count as isize).contains(&target) {
            let start = target as usize * page_size;
            if let Some(index) = (start..(start + page_size).min(count))
                .find(|&index| self.layout_candidate(index).is_some())
            {
                if target != current {
                    self.navigated = true;
                    self.engine.note_page_turn();
                }
                self.highlight = index;
                break;
            }
            target += step.signum();
        }
    }

    pub(super) fn candidate_count(&self) -> usize {
        match &self.composed {
            Some(Composed::Candidates { layout, .. }) => layout.len(),
            _ => 0,
        }
    }

    /// 候选布局里第 `index` 个（跨页下标）。
    pub(super) fn layout_candidate(&self, index: usize) -> Option<Candidate> {
        match &self.composed {
            Some(Composed::Candidates { layout, .. }) => layout.candidate(index).cloned(),
            _ => None,
        }
    }

    /// 当前排布里文本是 `text` 的第一个候选下标（跨页）：调频之后把高亮落回原来那个词，
    /// 连着按 K / J 调的还是同一个候选。
    pub(super) fn index_of_candidate(&self, text: &str) -> Option<usize> {
        match &self.composed {
            Some(Composed::Candidates { layout, .. }) => (0..layout.len())
                .find(|index| layout.candidate(*index).is_some_and(|c| c.text == text)),
            _ => None,
        }
    }

    pub(super) fn commit_index(&mut self, index: usize) -> Option<String> {
        let candidate = self.layout_candidate(index)?;
        // 这段拼音还没选完时 Engine 返回空串：选中的词留在那边等组句结束，这次不上屏
        let text = self.engine.commit(&candidate);
        (!text.is_empty()).then_some(text)
    }

    /// 按当前状态生成一帧：翻译评审优先；没在组句给空帧；否则给高亮所在的那一页。
    pub(super) fn current_frame(&self) -> Frame {
        match &self.composed {
            None => Frame::default(),
            Some(Composed::Raw { text, cursor }) => Frame {
                preedit: vec![PreeditSegment {
                    text: text.clone(),
                    kind: PreeditKind::Typed,
                }],
                cursor: *cursor,
                preedit_mode: self.config.preedit,
                candidates: CandidateList { items: Vec::new() },
                highlight: usize::MAX,
                page: 0,
                page_count: 1,
                layout: self.config.layout,
                theme: self.config.theme,
                aux_code_show: false,
                sentence: None,
                notice: self.notice.clone(),
                frequencies: Vec::new(),
            },
            Some(Composed::Candidates {
                preedit,
                cursor,
                layout,
            }) => {
                let page_size = self.config.page_size;
                let highlight = self.highlight.min(layout.len().saturating_sub(1));
                let page = highlight / page_size;
                let items: Vec<Candidate> = layout
                    .page(page)
                    .into_iter()
                    // 空文本只用于显示占位，不传给 Engine 上屏；保持数字标签与固定位置一致。
                    .map(|cell| {
                        cell.candidate().cloned().unwrap_or_else(|| Candidate {
                            text: String::new(),
                            kind: CandidateKind::Chinese,
                            syllables: Vec::new(),
                            reading: None,
                            translation: None,
                            aux_code: None,
                        })
                    })
                    .collect();
                let mut candidates = CandidateList { items };
                self.engine.annotate(&mut candidates);
                // 调频键按住时把每个候选的频次一起下发（面板照它画），空 vec 表示不显示
                let frequencies = if self.preview_frequency {
                    candidates
                        .items
                        .iter()
                        .map(|candidate| self.engine.frequency_of(candidate))
                        .collect()
                } else {
                    Vec::new()
                };
                Frame {
                    preedit: preedit.clone(),
                    cursor: *cursor,
                    preedit_mode: self.config.preedit,
                    candidates,
                    highlight: highlight - page * page_size,
                    page,
                    page_count: layout.pages().max(1),
                    layout: self.config.layout,
                    theme: self.config.theme,
                    aux_code_show: false,
                    sentence: self.sentence.clone(),
                    notice: self.notice.clone(),
                    frequencies,
                }
            }
        }
    }
}

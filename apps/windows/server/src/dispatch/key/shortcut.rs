//! 组句中的快捷键：修饰键 + 数字 (上屏译词、删候选) 与调频键 + J / K (升降当前候选)。与 macOS 壳对齐。

use qingjian_core::{Candidate, CandidateList};
use qingjian_platform::KeyCombo;
use qingjian_platform::protocol::{KeyEvent, KeyModifiers};

use super::{Effect, codes};
use crate::dispatch::Router;

impl Router {
    /// 配到哪组就上屏第一 / 第二个译词或删候选; 哪组都不是 (包括配成 `none` 关掉的那组) 返回 `None`, 按普通键处理.
    pub(super) fn apply_digit_shortcut(
        &mut self,
        digit: usize,
        chord: KeyModifiers,
    ) -> Option<Effect> {
        if chord == KeyModifiers::default() {
            return None;
        }
        let (first, second) = self.config.translation_keys;
        let effect = if Some(chord) == first {
            self.commit_translation_on_page(digit, 0)
        } else if Some(chord) == second {
            self.commit_translation_on_page(digit, 1)
        } else if Some(chord) == self.config.delete_keys {
            self.forget_on_page(digit)
        } else {
            return None;
        };
        Some(effect)
    }

    /// 调频修饰键（配置 `[shortcut] adjust_frequency`，缺省 Ctrl）的按下 / 抬起：只用来开 / 关候选窗口里的
    /// 频次预览，键本身照旧归应用（回 `Noted`）。没配调频键、没在组句、不是修饰键都返回 `None`，
    /// 按键继续按原来的规矩分流。`composing` 由调用方给（与 [`Self::apply_digit_shortcut`] 同一套）。
    pub(super) fn apply_adjust_modifier(
        &mut self,
        event: &KeyEvent,
        composing: bool,
    ) -> Option<Effect> {
        let keys = self.config.adjust_keys?;
        if !composing || !codes::is_modifier_key(event.virtual_key) {
            return None;
        }
        let preview = !event.release && event.modifiers.chord() == keys;
        if std::mem::replace(&mut self.preview_frequency, preview) != preview {
            tracing::debug!(preview, "频次预览");
        }
        Some(Effect::Noted)
    }

    /// 高亮上下挪一格 (配置 `[shortcut] highlight_down` / `highlight_up`, 缺省 Ctrl+N / Ctrl+P): 与 ↓ / ↑ 同义,
    /// 到页边自动翻页. 没配到的那一边返回 `None`, 键照旧按普通键分流.
    pub(super) fn apply_highlight_shortcut(&mut self, event: &KeyEvent) -> Option<Effect> {
        let typed = event.character?;
        let chord = event.modifiers.chord();
        let hit = |combo: Option<KeyCombo>| {
            combo.is_some_and(|combo| {
                KeyModifiers::from(combo.modifiers) == chord && combo.key.eq_ignore_ascii_case(&typed)
            })
        };
        let delta = if hit(self.config.highlight_down) {
            1
        } else if hit(self.config.highlight_up) {
            -1
        } else {
            return None;
        };
        self.move_highlight(delta);
        Some(Effect::Navigated)
    }

    /// 调频键 + J / K：把当前高亮的候选降 / 升一格。名次可能变，所以自己重新组一次句，
    /// 再把高亮落回这个词的新位置（连着按调的还是同一个候选）；不是这两个字母返回 `None`。
    pub(super) fn apply_frequency_shortcut(&mut self, event: &KeyEvent) -> Option<Effect> {
        let keys = self.config.adjust_keys?;
        if event.modifiers.chord() != keys {
            return None;
        }
        let up = codes::adjust_key(event.virtual_key)?;
        let Some(candidate) = self.layout_candidate(self.highlight) else {
            tracing::debug!("这一格没有候选，没什么可调");
            return Some(Effect::Navigated);
        };
        let change = self.engine.adjust_frequency(&candidate, up);
        let message = if change.changed {
            None
        } else if self.engine.frequency_of(&candidate).is_none() {
            Some(format!("「{}」没有可以调的词频", candidate.text))
        } else if up {
            Some(format!("「{}」没有可以升的记录", candidate.text))
        } else {
            Some(format!(
                "「{}」已经降到底了, 没有学习记录可降",
                candidate.text
            ))
        };
        match message {
            Some(message) => {
                tracing::info!(%message);
                self.notice = Some(message);
            }
            None => self.notice = None,
        }
        let text = candidate.text.clone();
        self.recompose();
        if let Some(index) = self.index_of_candidate(&text) {
            self.highlight = index;
        }
        Some(Effect::Navigated)
    }

    /// 上屏第 `digit` 个候选的第 `sense` 条译文；没有那条译文就吞掉按键不动。
    fn commit_translation_on_page(&mut self, digit: usize, sense: usize) -> Effect {
        let Some(candidate) = self.annotated_candidate_on_page(digit) else {
            tracing::debug!(digit, "这一格没有候选");
            return Effect::Navigated;
        };
        match self.engine.commit_translation(&candidate, sense) {
            Some(text) => Effect::Changed(Some(text)),
            None => {
                tracing::debug!(digit, sense, "这个候选没有这条译文");
                Effect::Navigated
            }
        }
    }

    /// 删第 `digit` 个候选（用户词整删、词库词清学习记录），提示随下一帧下发。
    fn forget_on_page(&mut self, digit: usize) -> Effect {
        let Some(candidate) = self.candidate_on_page(digit) else {
            tracing::debug!(digit, "这一格没有候选，没什么可删");
            return Effect::Navigated;
        };
        let forgotten = self.engine.forget(&candidate);
        let text = &candidate.text;
        let message = if forgotten.user_word {
            format!("已删除用户词「{text}」")
        } else if forgotten.learning {
            format!("已忘掉对「{text}」的学习记录")
        } else {
            format!("「{text}」是词库里的词，也没有学习记录，没什么可删")
        };
        tracing::info!(%message);
        self.notice = Some(message);
        Effect::Changed(None)
    }

    /// 当前页第 `digit` 个候选（1 起）。
    fn candidate_on_page(&self, digit: usize) -> Option<Candidate> {
        let page_size = self.config.page_size;
        let page = self.highlight / page_size;
        let index = page * page_size + digit.checked_sub(1)?;
        self.layout_candidate(index)
    }

    /// 同上，补上译文（布局里存的是查询原样，译文画页时才补）。
    fn annotated_candidate_on_page(&self, digit: usize) -> Option<Candidate> {
        let candidate = self.candidate_on_page(digit)?;
        let mut list = CandidateList {
            items: vec![candidate],
        };
        self.engine.annotate(&mut list);
        list.items.pop()
    }
}

//! 神经重打分: 整句转换的前几条路径交给第二打分来源 ([`SentenceScorer`]) 再排一次. 字级语言模型给整句 log 概率,
//! 决策模型 (`qingjian-decision`) 给同一批候选之间的相对优劣, 两种量纲在 [`Engine::rescore_paths`] 里分别处理.
//!
//! 打分有两种接法：同步的（[`Engine::with_sentence_scorer`]，查询里当场打，CLI 评测用）和异步的
//! （[`Engine::with_async_sentence_scorer`]，后台线程；壳里用）。两种都经过一张「前文 + 文本 → 神经分」的缓存
//! （[`NeuralCache`]）：同步时缺的分当场补进去，异步时缺的先记下来，壳在用户停顿后调 [`Engine::request_rescoring`]
//! 一次送去后台，[`Engine::poll_rescoring`] 收到结果后再查一次，这时全部路径的分都在缓存里，排序自然换成重排后的。
//! 按键回调永远不等模型：先按词级模型出候选，模型的意见晚几十毫秒到。

mod cache;
mod hint;
mod worker;

#[cfg(test)]
mod tests;

use super::*;

pub(crate) use cache::NeuralCache;
pub use hint::ModelHint;
pub(crate) use worker::RescoreWorker;

impl Engine {
    /// 接了重打分器（同步或异步）。
    pub fn has_sentence_scorer(&self) -> bool {
        self.sentence_scorer.is_some()
            || self.rescorer.as_ref().is_some_and(RescoreWorker::is_alive)
    }

    /// 给模型看的前文：壳给了应用里的光标前文就用它（[`Self::set_rescoring_context`]），
    /// 否则用本会话最近上屏的字符；长度按 `neural_context` 截。
    pub(super) fn rescoring_context(&self) -> String {
        if self.neural_context == 0 {
            return String::new();
        }
        match &self.rescoring_before {
            Some(before) => take_last_chars(before, self.neural_context),
            None => self.history.recent(self.neural_context).to_owned(),
        }
    }

    /// 私密输入期间不让 "会把前文发到本机之外" 的打分器 (云端决策模型) 干活: 一个字都不出去,
    /// 这一轮也不重排. 本地模型不受影响 (`is_remote` 为假).
    fn privacy_blocks_rescoring(&self) -> bool {
        self.private && self.scorer_remote
    }

    /// 壳告知应用里光标前的文本（每次查询前给；应用给不出就 `None`，退回本会话历史）。
    pub fn set_rescoring_context(&mut self, before: Option<String>) {
        self.rescoring_before = before;
    }

    /// 把几条整句路径按「路径分 + λ·(神经分 − 静态分)」重排。缓存里缺分的：同步打分器当场补，异步的先记下等壳来取；
    /// 有任何一条没分就不动顺序（半截重排比不重排还糟）。
    pub(super) fn rescore_paths(&self, paths: &mut [Conversion]) {
        if paths.len() < 2 || !self.has_sentence_scorer() || self.privacy_blocks_rescoring() {
            return;
        }
        let context = self.rescoring_context();
        let mut cache = self.neural_cache.borrow_mut();
        cache.ensure_context(&context);
        let mut missing: Vec<String> = Vec::new();
        for path in paths.iter() {
            if cache.get(&path.text).is_none() && !missing.contains(&path.text) {
                missing.push(path.text.clone());
            }
        }
        if !missing.is_empty() {
            match &self.sentence_scorer {
                Some(scorer) => {
                    let texts: Vec<&str> = missing.iter().map(String::as_str).collect();
                    let scores = scorer.score(&context, &texts);
                    if scores.len() != texts.len() {
                        return;
                    }
                    for (text, score) in texts.iter().zip(scores) {
                        cache.insert(text, score);
                    }
                }
                None => {
                    for text in &missing {
                        cache.want(text);
                    }
                    return;
                }
            }
        }
        let lambda = self.neural_weight;
        // 相对分 (决策模型这类只判 "哪条更顺" 的打分器) 只有同一批候选之间可比, 先按批内均值居中再叠加;
        // 绝对分 (字级语言模型的整句 log 概率) 与静态模型同量纲, 直接替换掉它.
        let mean = match self.scorer_form {
            ScoreForm::Absolute => 0.0,
            ScoreForm::Relative => {
                let sum: f64 = paths
                    .iter()
                    .map(|path| cache.get(&path.text).expect("filled above"))
                    .sum();
                sum / paths.len() as f64
            }
        };
        // 重排前的名次: 重排之后拿来算每条被抬 / 压了几名
        let before: Vec<String> = paths.iter().map(|path| path.text.clone()).collect();
        for path in paths.iter_mut() {
            let neural = cache.get(&path.text).expect("filled above");
            path.score += match self.scorer_form {
                ScoreForm::Absolute => lambda * (neural - path.static_score),
                ScoreForm::Relative => lambda * (neural - mean),
            };
        }
        paths.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // 记下每条候选的名次变化给壳标 ↑N / ↓N; 相对分带满量程时再顺手算置信度 (分 / 满量程)
        let scale = self.scorer_scale.filter(|scale| *scale > 0.0);
        let mut hints = self.model_hints.borrow_mut();
        hints.clear();
        for (after, path) in paths.iter().enumerate() {
            let shift = before
                .iter()
                .position(|text| *text == path.text)
                .map_or(0, |before| before as i16 - after as i16);
            let confidence = match (self.scorer_form, scale) {
                (ScoreForm::Relative, Some(scale)) => cache
                    .get(&path.text)
                    .map(|score| (score / scale).clamp(0.0, 1.0) as f32),
                _ => None,
            };
            hints.insert(path.text.clone(), ModelHint { shift, confidence });
        }
        drop(hints);
        self.last_rescored.set(true);
    }

    /// 某条文本最近一次重排的模型标注 (名次变化 + 置信度); 没接打分器或这条没参与重排时为 `None`.
    /// 壳拿它在候选旁标 `AI 76% ↑2` (字级模型没有置信度, 只标 `AI ↑2`).
    pub fn model_hint(&self, text: &str) -> Option<ModelHint> {
        self.model_hints.borrow().get(text).copied()
    }

    /// 最近一次查询里有整句路径还没拿到神经分：壳该在用户停顿后调 [`Self::request_rescoring`]。
    pub fn rescoring_pending(&self) -> bool {
        self.rescorer.is_some() && self.neural_cache.borrow().has_wanted()
    }

    /// 把攒着的文本送去后台打分。没接异步打分器或没什么要打的返回 `false`。
    pub fn request_rescoring(&mut self) -> bool {
        if self.privacy_blocks_rescoring() {
            return false;
        }
        let Some(worker) = &self.rescorer else {
            return false;
        };
        let mut cache = self.neural_cache.borrow_mut();
        let wanted = cache.take_wanted();
        if wanted.is_empty() {
            return false;
        }
        tracing::debug!(texts = wanted.len(), "神经重打分请求");
        worker.submit(cache.context().to_owned(), wanted);
        true
    }

    /// 收后台打好的分。有新分进了缓存返回 `true`，壳该重新 [`Self::query`] 一次；前文已经变了的结果丢掉。
    pub fn poll_rescoring(&mut self) -> bool {
        let Some(worker) = &self.rescorer else {
            return false;
        };
        let mut updated = false;
        while let Some(scored) = worker.poll() {
            let mut cache = self.neural_cache.borrow_mut();
            if scored.context != cache.context() || scored.scores.len() != scored.texts.len() {
                continue;
            }
            for (text, score) in scored.texts.iter().zip(scored.scores) {
                cache.insert(text, score);
            }
            updated = true;
        }
        updated
    }
}

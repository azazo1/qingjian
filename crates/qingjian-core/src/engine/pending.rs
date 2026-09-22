//! 延迟上屏: 组句还没结束时, 选中的词先留在 preedit 里, 等组句结束 (或壳要把这一键交给应用) 才真正上屏。
//!
//! 这样退格能把它们逐个拆回来 (rime 的 `ReopenPreviousSegment`), 不需要去删应用里已经插进去的字。
//! 只有缓冲区里还剩拼音的上屏 (还能接着往下选) 才延迟; 一段拼音选完的最后一次上屏当场交出去。

use super::Engine;
use super::LastCommit;
use super::commit::CommitChain;

/// 组句里已经选中、还没交给应用的词。退格按后进先出拆回。
pub(super) struct PendingWord {
    /// 上屏文本 (preedit 里显示在拼音前面的那段, 已经是给应用的形态)。
    pub text: String,

    /// 这次上屏吃掉的键: 拆回时原样还回缓冲区开头。
    pub keys: String,

    /// 上屏前的上屏链: 拆回时整个恢复 (上一个词与段内词序列一起退回去)。
    pub chain: CommitChain,

    /// 这次上屏记的学习: 拆回后按「删掉重选」的账处理, 改选了别的词时退回 (见 `Engine::apply_retraction`)。
    pub commit: LastCommit,
}

impl Engine {
    /// 还没交给应用的已选文本 (preedit 里显示在拼音前面)。没有时是空串。
    pub fn pending_text(&self) -> String {
        self.pending.iter().map(|word| word.text.as_str()).collect()
    }

    /// 组句里有没有已经选中、还没交给应用的词。
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// 把还没交给应用的已选文本交出来: 组句结束、这一键要交给应用、取消组句时都调。
    /// 交出去的词从这一刻算真的上屏 (退格撤销的账从这里开始记)。
    pub fn take_pending(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }
        let words = std::mem::take(&mut self.pending);
        let mut text = String::new();
        for word in words {
            text.push_str(&word.text);
            self.remember_commit(word.commit);
        }
        text
    }

    /// 退格拆回最后一次选择: 键还回缓冲区、上屏链退回上屏前, 这次上屏按「已经删掉」记账 (改选别的词时学习退回)。
    /// 没有可拆的返回 `false`。
    pub(super) fn undo_pending(&mut self) -> bool {
        let Some(word) = self.pending.pop() else {
            return false;
        };
        self.composition.prepend(&word.keys);
        self.chain = word.chain;
        let mut commit = word.commit;
        commit.erased = commit.chars;
        self.remember_commit(commit);
        true
    }
}

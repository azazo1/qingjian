/// 把键盘输入常用的 `lue` / `nue` 转成词库规范形式.
///
/// 词库用 `v` 表示 ü. 其他音节原样返回.
pub fn canonical_syllable(text: &str) -> &str {
    match text {
        "lue" => "lve",
        "nue" => "nve",
        _ => text,
    }
}

/// `xíng` → `xing`, `lǜ` → `lv`, 数字声调丢掉; 再走 [`canonical_syllable`].
pub fn strip_tone(reading: &str) -> String {
    let mut out = String::with_capacity(reading.len());
    for c in reading.chars() {
        let plain = match c {
            'ā' | 'á' | 'ǎ' | 'à' => 'a',
            'ē' | 'é' | 'ě' | 'è' | 'ê' | 'ế' | 'ề' => 'e',
            'ī' | 'í' | 'ǐ' | 'ì' => 'i',
            'ō' | 'ó' | 'ǒ' | 'ò' => 'o',
            'ū' | 'ú' | 'ǔ' | 'ù' => 'u',
            'ü' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => 'v',
            'ḿ' => 'm',
            'ń' | 'ň' | 'ǹ' => 'n',
            '\u{0300}'..='\u{036f}' => continue,
            c if c.is_ascii_digit() => continue,
            c => c.to_ascii_lowercase(),
        };
        out.push(plain);
    }
    canonical_syllable(&out).to_owned()
}

/// 一个音节的查询条件.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyllablePattern<'a> {
    /// 用户敲的字母：完整音节，或音节前缀 / 声母。
    pub text: &'a str,

    /// 为真时要求音节与 `text` 完全相等，否则只要求以 `text` 开头（简拼、未打完的音节）。
    pub complete: bool,
}

impl<'a> SyllablePattern<'a> {
    pub fn complete(text: &'a str) -> Self {
        Self {
            text,
            complete: true,
        }
    }

    pub fn prefix(text: &'a str) -> Self {
        Self {
            text,
            complete: false,
        }
    }

    pub fn accepts(&self, syllable: &str) -> bool {
        let text = canonical_syllable(self.text);
        let syllable = canonical_syllable(syllable);
        if self.complete {
            syllable == text
        } else {
            syllable.starts_with(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_keyboard_spelling_for_u_umlaut() {
        assert_eq!(canonical_syllable("lue"), "lve");
        assert_eq!(canonical_syllable("nue"), "nve");
        assert_eq!(canonical_syllable("lve"), "lve");
        assert_eq!(canonical_syllable("nve"), "nve");
        assert_eq!(canonical_syllable("xue"), "xue");
    }

    #[test]
    fn strips_tone_marks_and_digits() {
        assert_eq!(strip_tone("xíng"), "xing");
        assert_eq!(strip_tone("lǜ"), "lv");
        assert_eq!(strip_tone("nǚ"), "nv");
        assert_eq!(strip_tone("lüè"), "lve");
        assert_eq!(strip_tone("nüè"), "nve");
        assert_eq!(strip_tone("lue4"), "lve");
        assert_eq!(strip_tone("nue4"), "nve");
        assert_eq!(strip_tone("ā"), "a");
        assert_eq!(strip_tone("de"), "de");
    }

    #[test]
    fn accepts_keyboard_and_canonical_spellings_equally() {
        assert!(SyllablePattern::complete("lue").accepts("lve"));
        assert!(SyllablePattern::complete("nve").accepts("nue"));
        assert!(!SyllablePattern::complete("xue").accepts("xve"));
    }
}

//! 拼音写法归一: 带声调符号与带声调数字的都转成不带声调, ü 写 v 的小写音节.

pub use qingjian_dictionary::strip_tone;

/// 通用词表的 `wei4'shen2'me` → `["wei", "shen", "me"]`; `lv4` 保持 v. 空段跳过.
pub fn numeric_syllables(pinyin: &str) -> Vec<String> {
    pinyin
        .split('\'')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(strip_tone)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tone_marks_and_digits() {
        assert_eq!(strip_tone("xíng"), "xing");
        assert_eq!(strip_tone("lǜ"), "lv");
        assert_eq!(strip_tone("nǚ"), "nv");
        assert_eq!(strip_tone("lüè"), "lve");
        assert_eq!(strip_tone("nüè"), "nve");
        assert_eq!(strip_tone("lue4"), "lve");
        assert_eq!(strip_tone("nue4"), "nve");
        assert_eq!(strip_tone("shi4"), "shi");
        assert_eq!(strip_tone("de"), "de");
    }

    #[test]
    fn splits_numeric_pinyin() {
        assert_eq!(numeric_syllables("wei4'shen2'me"), ["wei", "shen", "me"]);
        assert_eq!(numeric_syllables("nv3'ren2"), ["nv", "ren"]);
        assert_eq!(numeric_syllables("zhe4'er"), ["zhe", "er"]);
    }
}

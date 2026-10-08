//! 菜单与偏好设置窗口的动作：只改 config.toml（或触发一次性操作），改完由 apply_config 统一生效。

use super::diagnostics::{copy_to_pasteboard, open_with_system};
use super::*;
use crate::preferences::DEFAULT_FONT_LABEL;
use qingjian_platform::{KeyBinding, MacSwitchKey, ShiftLetter, UpdateChannel};

impl Host {
    /// 写短语前读取文件；外部规则有变化时同步列表并请用户重新确认。
    fn phrases_are_current(&mut self) -> bool {
        let Some(path) = self.settings.path() else {
            return false;
        };
        match qingjian_platform::Config::load(path) {
            Ok(latest) if latest.custom_phrases == self.settings.config().custom_phrases => true,
            Ok(_) => {
                self.settings.reload();
                self.apply_config(false);
                let message = "规则已在其他地方修改，请重新确认后操作。";
                self.preferences.set_phrase_error(message);
                self.preferences.set_status(message);
                false
            }
            Err(error) => {
                self.preferences.set_phrase_error(&error.to_string());
                self.preferences.set_status(&error.to_string());
                false
            }
        }
    }

    /// 切换中文模式下的中文 / 英文标点 (输入法菜单那一项与 `[shortcut] punctuation_toggle` 快捷键都到这):
    /// 翻转 `[general] full_width_punctuation` 落盘, 热加载顺带刷新菜单勾选与菜单栏标题.
    pub fn toggle_punctuation(&mut self) {
        let on = !self.settings.config().general.full_width_punctuation;
        tracing::info!(on, "切换中文 / 英文标点");
        if self
            .settings
            .set_bool("general", "full_width_punctuation", on)
        {
            self.apply_config(false);
        }
    }

    /// 表格中的启用开关只修改所选规则。
    pub fn set_phrase_enabled(&mut self, index: usize, enabled: bool) {
        if !self.phrases_are_current() {
            return;
        }
        let mut phrases = self.settings.config().custom_phrases.clone();
        let Some(phrase) = phrases.get_mut(index) else {
            return;
        };
        phrase.enabled = enabled;
        let Some(path) = self.settings.path() else {
            return;
        };
        let result = qingjian_platform::Config::set_custom_phrases(path, &phrases);
        self.settings.reload();
        self.apply_config(false);
        if let Err(error) = result {
            self.preferences.set_status(&error);
        }
    }

    /// 菜单动作。开关类先落盘再热加载，菜单勾选状态永远来自文件里的值。
    pub fn perform(&mut self, action: MenuAction) {
        tracing::info!(?action, "菜单");
        match action {
            MenuAction::ToggleFuzzy(index) => {
                let name = FuzzyRules::NAMES[index];
                let on = !self.settings.config().fuzzy.is_on(name);
                if self.settings.set_bool("fuzzy", name, on) {
                    self.apply_config(false);
                }
            }
            MenuAction::TogglePunctuation => self.toggle_punctuation(),
            MenuAction::OpenPreferences => {
                self.preferences.sync_usage(
                    &self.engine.usage_summary(),
                    &self.engine.vocabulary_summary(),
                    self.learning_language,
                );
                self.preferences.show();
            }
            MenuAction::LearnPhrase => {
                self.engine.prepare_phrase_readings();
                self.learn_phrase.show();
            }
            MenuAction::OpenLogs => {
                if let Some(dir) = logging::log_dir() {
                    open_with_system(&[&dir.to_string_lossy()]);
                }
            }
            MenuAction::Restart => self.restart(),
            MenuAction::OpenDownload => open_with_system(&[qingjian_update::DOWNLOAD_URL]),
        }
    }

    /// 设置窗口里改了一个控件：写配置、热加载；写不成（非法组合、空文本）也要把控件同步回真实值。
    pub fn change_setting(&mut self, setting: Setting, value: SettingValue) {
        // 密钥值不进日志
        tracing::info!(?setting, "设置");
        let config = self.settings.config().clone();
        match (setting, value) {
            (Setting::NewPhrase, _) => {
                self.preferences.edit_phrase(&config, None);
                return;
            }
            (Setting::EditPhrase, _) => {
                if let Some(index) = self.preferences.selected_phrase() {
                    self.preferences.edit_phrase(&config, Some(index));
                }
                return;
            }
            (Setting::CancelPhraseEdit, _) => {
                self.preferences.close_phrase_editor();
                return;
            }

            (Setting::PhraseDraft, _) => return,
            (Setting::SelectPhrase, SettingValue::Index(index)) => {
                self.preferences.select_phrase(&config, index);
                return;
            }
            (Setting::SavePhrase | Setting::DeletePhrase, _) => {
                if !self.phrases_are_current() {
                    return;
                }
                let config = self.settings.config();
                let mut phrases = config.custom_phrases.clone();
                let mut saved_index = phrases.len();
                if setting == Setting::DeletePhrase {
                    let Some(index) = self.preferences.selected_phrase() else {
                        return;
                    };
                    if index >= phrases.len() {
                        return;
                    }
                    phrases.remove(index);
                } else {
                    let (index, draft) = match self.preferences.phrase_draft(config) {
                        Ok(value) => value,
                        Err(error) => {
                            self.preferences.set_phrase_error(&error);
                            self.preferences.set_status(&error);
                            return;
                        }
                    };
                    if let Some(index) = index {
                        let Some(phrase) = phrases.get_mut(index) else {
                            return;
                        };
                        *phrase = draft;
                        saved_index = index;
                    } else {
                        phrases.push(draft);
                    }
                }
                let Some(path) = self.settings.path() else {
                    return;
                };
                if let Err(error) = qingjian_platform::Config::set_custom_phrases(path, &phrases) {
                    self.preferences.set_phrase_error(&error);
                    self.preferences.set_status(&error);
                    return;
                }
                self.preferences.close_phrase_editor();
                self.settings.reload();
                self.apply_config(false);
                if setting == Setting::SavePhrase {
                    self.preferences
                        .select_phrase(self.settings.config(), saved_index + 1);
                }
                self.preferences.set_status("自定义短语已保存");
                return;
            }
            (Setting::FullWidthPunctuation, SettingValue::Index(index)) => {
                self.settings
                    .set_bool("general", "full_width_punctuation", index == 0);
            }
            (Setting::LearningLanguage, SettingValue::Index(index)) => {
                // 菜单最后一项是「不显示译文」
                let code = self
                    .languages
                    .get(index)
                    .map_or(LEARNING_LANGUAGE_OFF, |language| language.code());
                self.settings
                    .set_value("general", "learning_language", code);
            }
            (Setting::PageSize, SettingValue::Index(index)) => {
                self.settings
                    .set_value("general", "page_size", index as i64 + 1);
            }
            (Setting::PageKeys, SettingValue::Index(index)) => {
                if let Some(pair) = PAGE_KEY_OPTIONS.get(index) {
                    self.settings.set_value("general", "page_keys", *pair);
                }
            }
            (Setting::Theme, SettingValue::Index(index)) => {
                if let Some(theme) = ThemeMode::ALL.get(index) {
                    self.settings.set_value("general", "theme", theme.key());
                }
            }
            (Setting::Renderer, SettingValue::Index(index)) => {
                if let Some(renderer) = CandidateRenderer::ALL.get(index) {
                    self.settings
                        .set_value("general", "renderer", renderer.key());
                }
            }
            (Setting::Font, SettingValue::Text(text)) => {
                let font = text.trim();
                let font = if font == DEFAULT_FONT_LABEL { "" } else { font };
                self.settings.set_value("general", "font", font);
            }
            (Setting::Layout, SettingValue::Index(index)) => {
                if let Some(layout) = LayoutMode::ALL.get(index) {
                    self.settings.set_value("general", "layout", layout.key());
                }
            }
            (Setting::HorizontalGrid, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "horizontal_grid", on);
            }
            (Setting::Preedit, SettingValue::Index(index)) => {
                if let Some(mode) = PreeditMode::ALL.get(index) {
                    self.settings.set_value("general", "preedit", mode.key());
                }
            }
            (Setting::QuestionMark, SettingValue::Bool(on)) => {
                self.settings.set_bool("shortcut", "question_mark", on);
            }
            (Setting::ExpressionKey | Setting::QuestionKey, SettingValue::Index(index)) => {
                if let Some(&key) = ModeKeys::CANDIDATES.get(index) {
                    let mut keys = config.shortcut.mode.sanitized();
                    let name = if setting == Setting::ExpressionKey {
                        keys.expression = key;
                        "expression"
                    } else {
                        keys.question = key;
                        "question"
                    };
                    if keys.is_valid() {
                        self.settings.set_value("shortcut", name, key.to_string());
                    } else {
                        tracing::warn!("表达式键与问字键不能相同，未改");
                    }
                }
            }
            (
                Setting::TranslationKeys | Setting::TranslationSecondKeys,
                SettingValue::Text(text),
            ) => match text.parse::<KeyBinding<Modifiers>>() {
                Ok(chosen) => {
                    let (first, second) = config.shortcut.translation_keys();
                    let (name, other) = if setting == Setting::TranslationKeys {
                        ("translation", second)
                    } else {
                        ("translation_second", first)
                    };
                    // 两组都关着不算撞车; 只有两边都配着同一个修饰键才拦
                    let clash = matches!(
                        (chosen.key(), other.key()),
                        (Some(chosen), Some(other)) if chosen == other
                    );
                    if clash {
                        tracing::warn!("两组译词快捷键不能相同，未改");
                    } else {
                        self.settings
                            .set_value("shortcut", name, chosen.to_string());
                    }
                }
                Err(error) => tracing::warn!(%error, "修饰键组合不合法，未改"),
            },
            (Setting::DeleteCandidateKeys, SettingValue::Text(text)) => {
                match text.parse::<KeyBinding<Modifiers>>() {
                    Ok(chosen) => {
                        let (first, second) = config.shortcut.translation_keys();
                        let clash = chosen.key().is_some_and(|keys| {
                            first.key() == Some(keys) || second.key() == Some(keys)
                        });
                        if clash {
                            tracing::warn!("删候选的快捷键不能与译词快捷键相同，未改");
                        } else {
                            self.settings.set_value(
                                "shortcut",
                                "delete_candidate",
                                chosen.to_string(),
                            );
                        }
                    }
                    Err(error) => tracing::warn!(%error, "修饰键组合不合法，未改"),
                }
            }
            (Setting::AdjustFrequencyKeys, SettingValue::Text(text)) => {
                // 调频键不需要与别的快捷键比：它配的是「按住看频次」与 J / K，不占数字键
                match text.parse::<KeyBinding<Modifiers>>() {
                    Ok(chosen) => {
                        self.settings
                            .set_value("shortcut", "adjust_frequency", chosen.to_string());
                    }
                    Err(error) => tracing::warn!(%error, "修饰键组合不合法，未改"),
                }
            }
            (
                Setting::TranslateSelectionKeys
                | Setting::LearnPhraseKeys
                | Setting::PunctuationToggleKeys,
                SettingValue::Text(text),
            ) => match text.parse::<KeyBinding<KeyCombo>>() {
                Ok(chosen) => {
                    let name = match setting {
                        Setting::LearnPhraseKeys => "learn_phrase",
                        Setting::PunctuationToggleKeys => "punctuation_toggle",
                        _ => "translate_selection",
                    };
                    self.settings
                        .set_value("shortcut", name, chosen.to_string());
                }
                Err(error) => tracing::warn!(%error, "快捷键不合法，未改"),
            },
            (Setting::MacSwitchSingle, SettingValue::Bool(on)) => {
                self.settings.set_bool("shortcut", "mac_switch_single", on);
            }
            (Setting::MacSwitchDual, SettingValue::Bool(on)) => {
                self.settings.set_bool("shortcut", "mac_switch_dual", on);
            }
            (
                Setting::MacSwitchToggle | Setting::MacSwitchEnglish | Setting::MacSwitchChinese,
                SettingValue::Text(text),
            ) => match text.parse::<MacSwitchKey>() {
                Ok(key) => {
                    let name = match setting {
                        Setting::MacSwitchToggle => "mac_switch_toggle",
                        Setting::MacSwitchEnglish => "mac_switch_english",
                        _ => "mac_switch_chinese",
                    };
                    self.settings.set_value("shortcut", name, key.key_string());
                }
                Err(error) => tracing::warn!(%error, "切换键不合法，未改"),
            },
            (Setting::MacCapsLockSwitch, SettingValue::Bool(on)) => {
                self.settings
                    .set_bool("shortcut", "mac_caps_lock_switch", on);
            }
            (Setting::ModeBadge, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "mode_badge", on);
            }
            (Setting::ResetShortcuts, _) => {
                let defaults = ShortcutConfig::default();
                self.settings
                    .set_value("general", "page_keys", PAGE_KEY_OPTIONS[0]);
                self.settings.set_value(
                    "shortcut",
                    "expression",
                    defaults.mode.expression.to_string(),
                );
                self.settings
                    .set_value("shortcut", "question", defaults.mode.question.to_string());
                self.settings
                    .set_bool("shortcut", "question_mark", defaults.mode.question_mark);
                self.settings.set_value(
                    "shortcut",
                    "translation",
                    defaults.translation.to_string(),
                );
                self.settings.set_value(
                    "shortcut",
                    "translation_second",
                    defaults.translation_second.to_string(),
                );
                self.settings.set_value(
                    "shortcut",
                    "translate_selection",
                    defaults.translate_selection.to_string(),
                );
                self.settings.set_value(
                    "shortcut",
                    "learn_phrase",
                    defaults.learn_phrase.to_string(),
                );
                self.settings.set_value(
                    "shortcut",
                    "punctuation_toggle",
                    defaults.punctuation_toggle.to_string(),
                );
                self.settings.set_value(
                    "shortcut",
                    "delete_candidate",
                    defaults.delete_candidate.to_string(),
                );
                self.settings.set_value(
                    "shortcut",
                    "adjust_frequency",
                    defaults.adjust_frequency.to_string(),
                );
                self.settings
                    .set_bool("shortcut", "mac_switch_single", defaults.mac_switch_single);
                self.settings.set_value(
                    "shortcut",
                    "mac_switch_toggle",
                    defaults.mac_switch_toggle_key().key_string(),
                );
                self.settings
                    .set_bool("shortcut", "mac_switch_dual", defaults.mac_switch_dual);
                self.settings.set_value(
                    "shortcut",
                    "mac_switch_english",
                    defaults.mac_switch_dual_keys().0.key_string(),
                );
                self.settings.set_value(
                    "shortcut",
                    "mac_switch_chinese",
                    defaults.mac_switch_dual_keys().1.key_string(),
                );
                self.settings.set_bool(
                    "shortcut",
                    "mac_caps_lock_switch",
                    defaults.mac_caps_lock_switch,
                );
            }
            (Setting::DictionaryEnabled(index), SettingValue::Bool(on)) => {
                if let Some(info) = self.dictionary_list.get(index).cloned() {
                    if info.builtin {
                        self.set_domain_enabled(&info.stem, on);
                    } else {
                        self.set_dictionary_enabled(&info.stem, on);
                    }
                }
            }
            (Setting::DictionaryRemove(index), _) => {
                self.remove_dictionary(index);
                return;
            }
            (Setting::ImportDictionary, _) => {
                if let Some(path) = crate::preferences::choose_dictionary_file() {
                    self.import_dictionary(&path);
                }
                return;
            }
            (Setting::Fuzzy(index), SettingValue::Bool(on)) => {
                self.settings
                    .set_bool("fuzzy", FuzzyRules::NAMES[index], on);
            }
            (Setting::UpdateCheck, SettingValue::Bool(on)) => {
                self.settings.set_bool("update", "check", on);
            }
            (Setting::UpdateChannel, SettingValue::Index(index)) => {
                if let Some(channel) = UpdateChannel::ALL.get(index) {
                    self.settings.set_value("update", "channel", channel.key());
                }
            }
            (Setting::Traditional, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "traditional", on);
            }
            (Setting::EnglishCandidates, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "english_candidates", on);
            }
            (Setting::ChineseFirst, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "chinese_first", on);
            }
            (Setting::PunctuationFirst, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "punctuation_first", on);
            }
            (Setting::ShiftLetter, SettingValue::Bool(on)) => {
                let mode = if on {
                    ShiftLetter::Compose
                } else {
                    ShiftLetter::Passthrough
                };
                self.settings
                    .set_value("general", "shift_letter", mode.key());
            }
            // 勾上写缺省的终端 / 编辑器列表，去掉写空表；手改过的列表勾一下就回缺省
            (Setting::EnglishCandidatesOffInApps, SettingValue::Bool(on)) => {
                let apps: toml_edit::Array = if on {
                    DEFAULT_ENGLISH_CANDIDATES_OFF.iter().copied().collect()
                } else {
                    toml_edit::Array::new()
                };
                self.settings
                    .set_value("apps", "english_candidates_off", apps);
            }
            // 弹出菜单按 Scheme::ALL 的顺序。写的是 [general] scheme（旧键 shuangpin 已并入它）：
            // 写旧键的话，配置里 scheme 的缺省值非空、解析时优先，用户选的方案会被静默忽略。
            (Setting::Scheme, SettingValue::Index(index)) => {
                let key = Scheme::ALL
                    .get(index)
                    .map_or(Scheme::Pinyin.key(), |scheme| scheme.key());
                self.settings.set_value("general", "scheme", key);
            }
            (Setting::ShuangpinRawPreedit, SettingValue::Bool(on)) => {
                self.settings
                    .set_bool("general", "shuangpin_raw_preedit", on);
            }
            // 五笔：勾上就是 86 版，取消就是关。与上面的拼音方案同时开着就是混输。
            (Setting::Wubi, SettingValue::Bool(on)) => {
                self.settings
                    .set_value("general", "wubi", if on { "wubi86" } else { "" });
            }
            (Setting::OpenConfigFile, _) => {
                if let Some(path) = self.settings.path() {
                    open_with_system(&["-t", &path.to_string_lossy()]);
                }
                return;
            }
            (Setting::InputLog, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "input_log", on);
            }
            (Setting::Learning, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "learning", on);
            }
            (Setting::SystemTextReplacements, SettingValue::Bool(on)) => {
                self.settings
                    .set_bool("general", "system_text_replacements", on);
            }
            (Setting::ClearInputLog, _) => {
                self.clear_input_log();
                return;
            }
            (Setting::VerboseLog, SettingValue::Bool(on)) => {
                let level = if on { LogLevel::Debug } else { LogLevel::Info };
                self.settings.set_value("general", "log_level", level.key());
            }
            (Setting::CheckUpdateNow, _) => {
                if let Some(updates) = &self.updates {
                    updates.check_now(&self.settings.config().update);
                }
                self.sync_update();
                return;
            }
            (Setting::OpenDownload, _) => {
                open_with_system(&[qingjian_update::DOWNLOAD_URL]);
                return;
            }
            (Setting::OpenWebsite, _) => {
                open_with_system(&[crate::preferences::WEBSITE_URL]);
                return;
            }
            (Setting::OpenRepository, _) => {
                open_with_system(&[crate::preferences::REPOSITORY_URL]);
                return;
            }
            (Setting::OpenLogDirectory, _) => {
                if let Some(dir) = logging::log_dir() {
                    open_with_system(&[&dir.to_string_lossy()]);
                }
                return;
            }
            (Setting::CopyDiagnostics, _) => {
                copy_to_pasteboard(&self.diagnostics());
                self.preferences
                    .set_status("诊断信息已复制到剪贴板，粘贴给作者即可");
                return;
            }
            (Setting::ExportLogs, _) => {
                match logging::export_logs() {
                    Ok(zip) => {
                        open_with_system(&["-R", &zip.to_string_lossy()]);
                        self.preferences
                            .set_status("日志已打包到桌面，发给作者即可");
                    }
                    Err(error) => self
                        .preferences
                        .set_status(&format!("打包日志失败：{error}")),
                }
                return;
            }
            (setting, value) => tracing::warn!(?setting, ?value, "设置项与控件值不匹配"),
        }
        self.apply_config(false);
    }
}

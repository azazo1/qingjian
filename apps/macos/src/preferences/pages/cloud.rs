//! 「云服务」页：本地整句模型开关，决策模型（laya / jev）开关与后端配置，云联想开关、云端词格数、
//! 接口地址 / 模型 / 密钥、测试连接。

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSButton, NSPopUpButton, NSSecureTextField, NSTextField};
use objc2_foundation::NSString;
use qingjian_decision::BackendKind;
use qingjian_platform::Config;

use crate::preferences::controls::{
    button, checkbox, note, row_checkbox, row_control, row_popup, secure_field, select,
    set_checked, text_field,
};
use crate::preferences::layout::{Layout, PAGE_PADDING, ROW_HEIGHT};
use crate::preferences::setting::Setting;
use crate::preferences::target::PreferencesTarget;

/// 云端词槽位弹出菜单的上限（配置文件里可以填更大，菜单只列到这）。
const MAX_CLOUD_SLOTS: usize = 4;

pub struct CloudPage {
    /// 本地整句模型开关。
    local_model: Retained<NSButton>,

    /// 决策模型开关。
    decision: Retained<NSButton>,

    /// 决策模型后端（本地服务 / 云端）。
    decision_backend: Retained<NSPopUpButton>,

    /// 决策模型接口地址。
    decision_endpoint: Retained<NSTextField>,

    /// 决策模型的密钥输入框（只有云端后端用），永远不回显已有值。
    decision_key: Retained<NSSecureTextField>,

    /// 云联想开关。
    enabled: Retained<NSButton>,

    /// 云端词槽位数（0–4）。
    slots: Retained<NSPopUpButton>,

    /// 接口地址。
    base_url: Retained<NSTextField>,

    /// 模型名。
    model: Retained<NSTextField>,

    /// 密钥输入框，永远不回显已有值。
    api_key: Retained<NSSecureTextField>,

    /// 「测试连接」按钮。
    test: Retained<NSButton>,
}

impl CloudPage {
    pub fn build(layout: &mut Layout, mtm: MainThreadMarker, target: &PreferencesTarget) -> Self {
        let local_model = checkbox(mtm, "本地整句模型", Setting::LocalModelEnabled, target);
        row_checkbox(layout, &local_model);
        note(
            layout,
            mtm,
            "随包的小模型在本机给整句候选重新排序，全程离线；停键后几十毫秒生效。关掉只用词库统计。",
        );
        let decision = checkbox(mtm, "决策模型重排", Setting::DecisionEnabled, target);
        row_checkbox(layout, &decision);
        note(
            layout,
            mtm,
            "把整句候选交给决策模型挑最顺的一条：本地服务全程离线，云端会把光标前文发出去。开着时上面的本地整句模型不再生效（两者占同一个位置）。",
        );
        let backend_titles: Vec<String> = BackendKind::ALL
            .iter()
            .map(|kind| kind.label().to_owned())
            .collect();
        let decision_backend = row_popup(
            layout,
            mtm,
            "决策后端",
            &backend_titles,
            Setting::DecisionBackend,
            target,
        );
        let decision_endpoint = text_field(mtm, Setting::DecisionEndpoint, target);
        row_control(layout, mtm, "决策接口", &decision_endpoint);
        let decision_key = secure_field(mtm, Setting::DecisionApiKey, target);
        row_control(layout, mtm, "决策密钥", &decision_key);
        note(
            layout,
            mtm,
            "接口地址留空用后端的缺省值（本地服务 http://127.0.0.1:8080/api/predict，云端 https://api.typesafe.ai/v1/systemone）；密钥只有云端用得上，填进这里会写进配置目录的 .env。",
        );
        let enabled = checkbox(mtm, "启用云联想", Setting::CloudEnabled, target);
        row_checkbox(layout, &enabled);
        note(
            layout,
            mtm,
            "开启后组句时会把光标附近的几十个字发给下面的服务，让模型补全整句、联想下文；密码框里绝不发送。菜单栏图标旁会带一个云朵。",
        );
        let slot_titles: Vec<String> = (0..=MAX_CLOUD_SLOTS)
            .map(|n| match n {
                0 => "不要（只要整句补全）".to_owned(),
                n => format!("{n} 格"),
            })
            .collect();
        let slots = row_popup(
            layout,
            mtm,
            "云端词位置",
            &slot_titles,
            Setting::CloudSlots,
            target,
        );
        note(
            layout,
            mtm,
            "云端词到了只补进第一页末尾这几格（比如 2 就是 8、9），前面的本地候选不动；没到就什么都不变，翻页后全是本地候选。",
        );
        let base_url = text_field(mtm, Setting::BaseUrl, target);
        row_control(layout, mtm, "接口地址", &base_url);
        let model = text_field(mtm, Setting::Model, target);
        row_control(layout, mtm, "模型", &model);
        let api_key = secure_field(mtm, Setting::ApiKey, target);
        row_control(layout, mtm, "API 密钥", &api_key);
        note(
            layout,
            mtm,
            "文本框按回车保存。密钥只保存在这台电脑上，不会随配置文件导出，也不显示已填的值。",
        );
        let test = button(mtm, "测试连接", Setting::TestCloud, target);
        layout.place(&test, PAGE_PADDING, 120.0, ROW_HEIGHT + 4.0);
        layout.next_row(ROW_HEIGHT + 4.0);
        note(
            layout,
            mtm,
            "用上面填的地址、模型、密钥发一条最小请求，结果显示在窗口底部。输入法进程看不到终端里的代理变量，走不通时先查这个。",
        );
        Self {
            local_model,
            decision,
            decision_backend,
            decision_endpoint,
            decision_key,
            enabled,
            slots,
            base_url,
            model,
            api_key,
            test,
        }
    }

    /// `key_present` 是云联想的密钥已经有了（环境或配置里）；密钥框永远不回显值，只换占位文字。
    /// `decision_key_present` 同理，是决策模型（jev）的密钥。`model_present` 是包里或用户目录里有模型文件，
    /// 没有就把本地模型的勾选灰掉；云联想关着时它下面的项全灰。
    pub fn sync(
        &self,
        config: &Config,
        key_present: bool,
        decision_key_present: bool,
        model_present: bool,
    ) {
        set_checked(&self.local_model, config.model.enabled && model_present);
        self.local_model.setEnabled(model_present);
        let decision = config.decision.enabled;
        set_checked(&self.decision, decision);
        self.decision_backend.setEnabled(decision);
        self.decision_endpoint.setEnabled(decision);
        // 密钥只有云端后端用
        self.decision_key
            .setEnabled(decision && config.decision.backend.needs_key());
        select(
            &self.decision_backend,
            BackendKind::ALL
                .iter()
                .position(|kind| *kind == config.decision.backend),
        );
        self.decision_endpoint
            .setStringValue(&NSString::from_str(&config.decision.endpoint));
        self.decision_key.setStringValue(&NSString::from_str(""));
        let decision_hint = if decision_key_present {
            "已设置，输入新值可替换"
        } else {
            "未设置"
        };
        self.decision_key
            .setPlaceholderString(Some(&NSString::from_str(decision_hint)));
        set_checked(&self.enabled, config.predict.enabled);
        let cloud = config.predict.enabled;
        self.slots.setEnabled(cloud);
        self.base_url.setEnabled(cloud);
        self.model.setEnabled(cloud);
        self.api_key.setEnabled(cloud);
        self.test.setEnabled(cloud);
        select(&self.slots, Some(config.predict.slots.min(MAX_CLOUD_SLOTS)));
        self.base_url
            .setStringValue(&NSString::from_str(&config.predict.base_url));
        self.model
            .setStringValue(&NSString::from_str(&config.predict.model));
        self.api_key.setStringValue(&NSString::from_str(""));
        let hint = if key_present {
            "已设置，输入新值可替换"
        } else {
            "未设置"
        };
        self.api_key
            .setPlaceholderString(Some(&NSString::from_str(hint)));
    }
}

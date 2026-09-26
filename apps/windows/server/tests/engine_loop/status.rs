//! 悬浮状态条随模式、双拼方案与开关变化。

use crate::support::*;

#[test]
fn status_bar_mode_click_changes_the_global_mode() {
    let config = RouterConfig {
        status_enabled: true,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });

    // 点「中」：状态条翻成「英」，之后每个 DLL 来取都拿到英文（全局一份，不是取一次就清）。
    router.handle_status_event(StatusEvent::ToggleMode);
    assert_eq!(recorder.calls().last(), Some(&Some("英".to_owned())));
    assert_eq!(synced_mode(&mut router, SESSION), Some(true));
    assert_eq!(synced_mode(&mut router, SESSION), Some(true));
    assert_eq!(synced_mode(&mut router, SessionId(2)), Some(true));
}

#[test]
fn status_bar_mode_click_is_ignored_when_builtin_english_is_off() {
    let config = RouterConfig {
        status_enabled: true,
        english_mode: false,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });
    assert_eq!(recorder.calls().last(), Some(&Some("中".to_owned())));

    // 关掉内置英文模式：点「中」不翻成「英」，DLL 取到的也是中文（DLL 那边同样会拦）
    router.handle_status_event(StatusEvent::ToggleMode);
    assert_eq!(recorder.calls().last(), Some(&Some("中".to_owned())));
    assert_eq!(synced_mode(&mut router, SESSION), Some(false));
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: true,
    });
    assert_eq!(synced_mode(&mut router, SESSION), Some(false));
}

#[test]
fn status_bar_follows_mode_when_enabled() {
    let config = RouterConfig {
        status_enabled: true,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    // 中文 → 英文：各刷一次；会话关掉（应用退出）不收；切成别的输入法才收起。
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: true,
    });
    router.handle(ClientMessage::CloseSession { session: SESSION });
    assert_eq!(
        recorder.calls(),
        vec![Some("中".to_owned()), Some("英".to_owned())]
    );

    router.handle(ClientMessage::ImeSwitched { session: SESSION });
    assert_eq!(recorder.calls().last(), Some(&None));
}

#[test]
fn status_bar_shows_shuangpin_scheme_in_chinese() {
    let config = RouterConfig {
        status_enabled: true,
        scheme: Scheme::Shuangpin(ShuangpinScheme::Xiaohe),
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });

    assert_eq!(recorder.calls(), vec![Some("中 · 小鹤双拼".to_owned())]);
}

#[test]
fn status_bar_stays_hidden_when_disabled() {
    let mut router = router();
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });

    assert_eq!(recorder.calls(), vec![None]);
}

#[test]
fn indicator_menu_toggles_status_bar() {
    use qingjian_platform::protocol::IndicatorCommand;

    let mut router = router();
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });

    // 任务栏图标菜单里点「悬浮状态条」：关着的打开，再点收起。
    let toggle = ClientMessage::Indicator {
        session: SESSION,
        command: IndicatorCommand::ToggleStatusBar,
    };
    router.handle(toggle.clone());
    assert_eq!(recorder.calls().last(), Some(&Some("中".to_owned())));
    router.handle(toggle);
    assert_eq!(recorder.calls().last(), Some(&None));
}

#[test]
fn mode_is_shared_by_every_app() {
    let config = RouterConfig {
        status_enabled: true,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));
    let other_app = SessionId(2);

    // 一个应用里切到英文：别的应用、之后新开的应用来取都是英文。
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: true,
    });
    assert_eq!(synced_mode(&mut router, other_app), Some(true));
    assert_eq!(synced_mode(&mut router, SessionId(3)), Some(true));

    // 切成别的输入法收起状态条；再有应用来取模式（又切回青简）就重新显示，模式照旧。
    router.handle(ClientMessage::ImeSwitched { session: SESSION });
    assert_eq!(recorder.calls().last(), Some(&None));
    assert_eq!(synced_mode(&mut router, other_app), Some(true));
    assert_eq!(recorder.calls().last(), Some(&Some("英".to_owned())));
}

/// 会话取一次 `SyncMode`，返回它拿到的全局模式。
fn synced_mode(router: &mut Router, session: SessionId) -> Option<bool> {
    match router.handle(ClientMessage::SyncMode { session }) {
        Some(ServerMessage::ModeSync { english, .. }) => english,
        other => panic!("SyncMode 应回 ModeSync，实际 {other:?}"),
    }
}

/// 模式徽标只在模式真的变了的那一下闪。
#[test]
fn mode_badge_flashes_only_when_the_mode_really_changes() {
    let config = RouterConfig {
        mode_badge: true,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    // 第一次报「中文」：本来就是中文，没变，不闪。
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });
    assert!(recorder.badges().is_empty(), "没变模式不该闪");

    // 切成英文：闪一下；还没报过光标矩形，锚点是 `None`（UI 线程拿鼠标位置兜底）。
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: true,
    });
    assert_eq!(recorder.badges(), vec![(true, None)]);

    // 再报一次英文：没变，不闪。
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: true,
    });
    assert_eq!(recorder.badges().len(), 1);

    // 状态条上点「英」切回中文：闪一下。
    router.handle_status_event(StatusEvent::ToggleMode);
    assert_eq!(recorder.badges().last(), Some(&(false, None)));
}

/// 徽标锚点跟着最近的光标矩形，组句结束后也留着。
#[test]
fn mode_badge_follows_the_last_caret_rect() {
    let config = RouterConfig {
        mode_badge: true,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    // 先敲一个键让这个会话成为聚焦会话，DLL 报来光标矩形后徽标按它摆。
    type_letters(&mut router, "ni");
    router.handle(ClientMessage::PositionCandidates {
        session: SESSION,
        rect: rect(),
    });
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: true,
    });
    assert_eq!(recorder.badges(), vec![(true, Some(rect()))]);

    // 组句收掉（空帧）之后光标矩形仍留着：徽标还摆在同一处。
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });
    assert_eq!(recorder.badges().last(), Some(&(false, Some(rect()))));
}

/// 关掉 `[general] mode_badge` 就不闪。
#[test]
fn mode_badge_is_silent_when_turned_off() {
    let config = RouterConfig {
        mode_badge: false,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: true,
    });
    assert!(recorder.badges().is_empty());
}

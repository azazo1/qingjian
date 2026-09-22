//! 本地整句模型：后台加载、停顿后请求重排、结果到了重画当前页。
//!
//! 按键回调里永远只跑词级模型；模型的意见在停键 80 毫秒后请求、二三十毫秒后到，只换候选窗口里的整句候选，
//! 用户翻过页或动过高亮就不打扰。前文优先用应用里光标前的文字（`refresh` 每次查询前给 Engine），应用给不出退回本会话历史。
//!
//! 占 Engine 里 "整句重排的第二打分来源" 这个位置的有两个: 本地字级模型 (`[model]`) 与决策模型 (`[decision]`,
//! jev / laya). 两者互斥, 由 [`Host::apply_rescorer`] 按配置挑一个接上.

use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;

use qingjian_decision::DecisionScorer;
use qingjian_neural::{CharScorer, NeuralError};

mod rescore_monitor;

pub(super) use rescore_monitor::{DEFAULT_MAX_WAIT, RescoreMonitor};

use super::*;

impl Host {
    /// 在后台线程加载模型并预热（第一次前向要编译 Metal 内核，几百毫秒），加载完由 [`Self::attach_loaded_model`] 接上。
    /// 没有模型文件就什么都不做。
    pub(super) fn load_local_model(&mut self) {
        if self.model_loader.is_some() || self.engine.has_sentence_scorer() {
            return;
        }
        let Some(path) = paths::model_path() else {
            tracing::info!("没有本地整句模型文件，不重排");
            return;
        };
        let (tx, rx) = channel::<Result<CharScorer, NeuralError>>();
        let spawned = std::thread::Builder::new()
            .name("qingjian-model-load".to_owned())
            .spawn(move || {
                let started = std::time::Instant::now();
                let loaded = CharScorer::load(&path).and_then(|scorer| {
                    scorer.score("", &["的"])?;
                    Ok(scorer)
                });
                if loaded.is_ok() {
                    tracing::info!(
                        path = %path.display(),
                        total_ms = started.elapsed().as_millis(),
                        "本地整句模型已加载并预热"
                    );
                }
                let _ = tx.send(loaded);
            });
        match spawned {
            Ok(_) => {
                self.model_loader = Some(rx);
                self.rescore.watch_loading();
            }
            Err(error) => tracing::warn!(%error, "起不了模型加载线程，本地整句模型不用"),
        }
    }

    /// 加载线程有结果了就接到 Engine 上；每次查询和加载定时器都会看一眼，不阻塞。
    pub fn attach_loaded_model(&mut self) {
        let Some(rx) = &self.model_loader else {
            self.rescore.stop_watching();
            return;
        };
        match rx.try_recv() {
            Ok(Ok(scorer)) => {
                self.engine
                    .set_async_sentence_scorer(Some(Box::new(scorer)));
                self.model_loader = None;
                self.rescore.stop_watching();
                // 模型上线了：日志里补一条会话信息，之后的条目知道重排开着
                let version = self.version.clone();
                self.engine.log_session(&version, "macos");
                self.rescore_current_round();
            }
            Ok(Err(error)) => {
                tracing::warn!(%error, "本地整句模型加载失败，不重排");
                self.model_loader = None;
                self.rescore.stop_watching();
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.model_loader = None;
                self.rescore.stop_watching();
            }
        }
    }

    /// 模型接上时用户正在组句：这一轮的查询从没见过打分器，不补查一次就永远错过重排。
    /// 补查只为攒下整句路径、起防抖，不动画面；用户已翻页或动过高亮就不打扰。
    fn rescore_current_round(&mut self) {
        if self.engine.composition().is_empty()
            || self.translation.is_some()
            || self.session.page != 0
            || self.session.navigated
        {
            return;
        }
        if self.engine.query().is_ok() {
            tracing::info!("模型接上时正在组句，补一轮重排");
            self.schedule_rescoring();
        }
    }

    /// 模型还在后台加载。
    pub fn model_loading(&self) -> bool {
        self.model_loader.is_some()
    }

    /// 卸掉模型（配置关掉）。
    pub(super) fn unload_local_model(&mut self) {
        self.model_loader = None;
        self.engine.set_async_sentence_scorer(None);
        self.rescore.stop_watching();
        self.rescore.stop();
    }

    /// 装配整句重排的来源: `[decision]` 开着就用决策模型, 否则 `[model]` 开着就用本地整句模型, 都没开就不重排.
    /// 两者占 Engine 里同一个位置, 所以先卸掉再装. 决策模型只建 HTTP 客户端与运行时, 构造很快, 不用另起线程;
    /// 它起不来 (没密钥, 地址不合法) 时改用本地模型, 别让用户两头落空.
    pub(super) fn apply_rescorer(&mut self, config: &qingjian_platform::Config) {
        self.unload_local_model();
        if config.decision.enabled {
            match DecisionScorer::new(&config.decision) {
                Ok(scorer) => {
                    tracing::info!(
                        backend = config.decision.backend.key(),
                        endpoint = config.decision.endpoint(),
                        timeout_ms = config.decision.timeout_ms,
                        "决策模型接上整句重排"
                    );
                    // 云端一次判断要几秒: 轮询的上限跟着它的请求超时放宽 (多留一秒收尾),
                    // 否则结果回来时壳已经停止收结果了
                    self.rescore.set_max_wait(Duration::from_millis(
                        config.decision.timeout_ms.saturating_add(1_000),
                    ));
                    self.engine
                        .set_async_sentence_scorer(Some(Box::new(scorer)));
                    self.rescore_current_round();
                    return;
                }
                Err(error) => tracing::warn!(%error, "决策模型起不来, 改用本地整句模型"),
            }
        }
        // 本地模型二三十毫秒就回, 等待上限用缺省的兜底值
        self.rescore.set_max_wait(DEFAULT_MAX_WAIT);
        if config.model.enabled {
            self.load_local_model();
        }
    }

    /// 每次查询之后：有整句路径等着打分就起防抖计时。
    pub fn schedule_rescoring(&mut self) {
        if self.engine.rescoring_pending() {
            self.rescore.schedule();
        }
    }

    /// 防抖到点：把攒着的整句路径送去后台，开始轮询。
    pub fn start_rescoring(&mut self) {
        if self.engine.composition().is_empty() {
            self.rescore.stop();
            return;
        }
        if self.engine.request_rescoring() {
            self.rescore.start_polling();
        }
    }

    /// 轮询到点：分回来了就重查一次、重画当前页；用户已翻页或动过高亮就只留着分不动画面。
    pub fn poll_rescoring(&mut self) {
        if self.engine.composition().is_empty() || self.translation.is_some() {
            self.rescore.stop();
            return;
        }
        if !self.engine.poll_rescoring() {
            // 等太久多半是前文变了（上屏后接着打下一段）、结果作废；真卡住也只是这轮不重排
            if self.rescore.expired() {
                tracing::debug!("等重排结果超时, 本轮不重排");
                self.rescore.stop();
            }
            return;
        }
        self.rescore.stop();
        if self.session.page != 0 || self.session.navigated {
            return;
        }
        let Ok(mut query) = self.engine.query() else {
            return;
        };
        self.engine.annotate(&mut query.candidates);
        let preedit = Preedit::from_marked(&query.marked_segments(), query.marked_cursor());
        let cloud = self.session.layout.cloud().to_vec();
        self.reset_session(preedit, query.candidates.items);
        if !cloud.is_empty() {
            self.session.layout.set_cloud(cloud);
        }
        self.render();
    }
}

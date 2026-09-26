//! 只报一次插入点的只读编辑会话：切中 / 英时用，让 Server 知道光标在哪 —— 模式徽标按它摆。
//! 组句期间的定位仍走 [`crate::com::composition`]，那边量的是组句范围。

use windows::Win32::UI::TextServices::{
    ITfContext, ITfEditSession, ITfEditSession_Impl, TF_ES_READ,
};
use windows::core::{Result, implement};

use super::anchor::caret_rect;
use crate::com::log::log;
use crate::com::service::SharedClient;

#[implement(ITfEditSession)]
pub(crate) struct CaretSession {
    /// 目标文档上下文。
    context: ITfContext,

    /// 引擎层：把插入点矩形发给 Server。
    engine: SharedClient,
}

impl ITfEditSession_Impl for CaretSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        // 没有选区 / 量不到时 `caret_rect` 自己退到鼠标处，这里不再另判。
        let rect = caret_rect(&self.context, ec);
        if let Ok(mut guard) = self.engine.try_borrow_mut()
            && let Some(client) = guard.as_mut()
            && let Err(error) = client.position_candidates(rect)
        {
            log(&format!("上报光标位置失败: {error}"));
        }
        Ok(())
    }
}

/// 请求一个异步只读会话报一次插入点。`Ok` 只说明已受理。
pub(crate) fn request_caret(
    context: &ITfContext,
    client_id: u32,
    engine: SharedClient,
) -> Result<()> {
    let session = CaretSession {
        context: context.clone(),
        engine,
    };
    super::update::request(context, client_id, session.into(), TF_ES_READ)
}

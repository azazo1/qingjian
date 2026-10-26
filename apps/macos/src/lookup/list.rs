//! 部件查字的结果列表: 一列「序号 + 汉字」, 双击或 Cmd+C 复制选中的字.
//!
//! 列表只负责显示与选择; 显隐、位置与复制后的提示由查字窗口处理.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBorderType, NSColor, NSControlTextEditingDelegate, NSFont, NSPasteboard,
    NSPasteboardTypeString, NSScrollView, NSTableColumn, NSTableView, NSTableViewDataSource,
    NSTableViewDelegate, NSTextAlignment, NSTextField, NSView,
};
use objc2_foundation::{NSInteger, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::target::LookupTarget;

/// 每行高度: 汉字用 16 号, 上下留一点空.
const ROW_H: f64 = 26.0;

/// 回显行高度.
const ECHO_H: f64 = 18.0;

/// 回显行与列表之间的空隙.
const ECHO_GAP: f64 = 4.0;

/// 序号那一块的宽度.
const NUMBER_W: f64 = 28.0;

/// 滚动条与边框让出的宽度.
const SCROLL_INSET: f64 = 20.0;

define_class!(
    // SAFETY: 只在主线程读写; ivars 只是这一屏字的快照.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<Vec<char>>]
    /// 部件查字结果的表格数据源.
    struct LookupRows;

    unsafe impl NSObjectProtocol for LookupRows {}
    // NSTableViewDelegate 的父 trait; 这里不接文本编辑的回调
    unsafe impl NSControlTextEditingDelegate for LookupRows {}

    unsafe impl NSTableViewDataSource for LookupRows {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn number_of_rows(&self, _table: &NSTableView) -> NSInteger {
            self.ivars().borrow().len() as NSInteger
        }
    }

    unsafe impl NSTableViewDelegate for LookupRows {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for_row(
            &self,
            _table: &NSTableView,
            column: Option<&NSTableColumn>,
            index: NSInteger,
        ) -> Option<Retained<NSView>> {
            self.cell(column, index)
        }
    }
);

impl LookupRows {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(RefCell::new(Vec::new()));
        unsafe { msg_send![super(this), init] }
    }

    /// 换成新的一批字, 旧的选中行随即作废.
    fn set_rows(&self, rows: Vec<char>) {
        *self.ivars().borrow_mut() = rows;
    }

    fn row(&self, index: NSInteger) -> Option<char> {
        let index = usize::try_from(index).ok()?;
        self.ivars().borrow().get(index).copied()
    }

    /// 一行的样子: 左边小号灰色序号, 右边汉字.
    fn cell(&self, column: Option<&NSTableColumn>, index: NSInteger) -> Option<Retained<NSView>> {
        let zi = self.row(index)?;
        let column = column?;
        let mtm = MainThreadMarker::from(self);
        let width = column.width();
        let view = NSView::initWithFrame(
            mtm.alloc(),
            NSRect::new(NSPoint::ZERO, NSSize::new(width, ROW_H)),
        );

        let number =
            NSTextField::labelWithString(&NSString::from_str(&(index + 1).to_string()), mtm);
        number.setFrame(NSRect::new(
            NSPoint::new(2.0, (ROW_H - ECHO_H) / 2.0),
            NSSize::new(NUMBER_W, ECHO_H),
        ));
        number.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        number.setTextColor(Some(&NSColor::secondaryLabelColor()));
        number.setAlignment(NSTextAlignment::Right);
        view.addSubview(&number);

        let han = NSTextField::labelWithString(&NSString::from_str(&zi.to_string()), mtm);
        han.setFrame(NSRect::new(
            NSPoint::new(NUMBER_W + 8.0, 2.0),
            NSSize::new(width - NUMBER_W - 12.0, ROW_H - 4.0),
        ));
        han.setFont(Some(&NSFont::systemFontOfSize(16.0)));
        view.addSubview(&han);

        Some(view)
    }
}

/// 部件查字的结果列表, 连回显行一起放进查字窗口的结果区.
pub(super) struct LookupList {
    echo: Retained<NSTextField>,
    scroll: Retained<NSScrollView>,
    table: Retained<NSTableView>,
    rows: Retained<LookupRows>,
}

impl LookupList {
    /// `area` 是结果区: 回显行贴着它的上边, 列表占剩下的部分.
    /// `target` 收双击: 双击一行就复制这个字.
    pub(super) fn new(mtm: MainThreadMarker, area: NSRect, target: &LookupTarget) -> Self {
        let rows = LookupRows::new(mtm);

        let echo = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        echo.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        echo.setTextColor(Some(&NSColor::secondaryLabelColor()));
        echo.setFrame(NSRect::new(
            NSPoint::new(area.origin.x, area.origin.y + area.size.height - ECHO_H),
            NSSize::new(area.size.width, ECHO_H),
        ));

        let table = NSTableView::initWithFrame(mtm.alloc(), NSRect::ZERO);
        table.setRowHeight(ROW_H);
        table.setAllowsMultipleSelection(false);
        table.setAllowsEmptySelection(true);
        table.setHeaderView(None);
        let column = NSTableColumn::initWithIdentifier(mtm.alloc(), &NSString::from_str("zi"));
        column.setWidth(area.size.width - SCROLL_INSET);
        table.addTableColumn(&column);
        // SAFETY: 数据源与代理就是 rows, 与表格一起活到下一次查询
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(&*rows)));
            table.setDelegate(Some(ProtocolObject::from_ref(&*rows)));
            // 双击一行: copySelected: 定义在 LookupTarget 上
            table.setTarget(Some(target));
            table.setDoubleAction(Some(sel!(copySelected:)));
        }

        let list_height = area.size.height - ECHO_H - ECHO_GAP;
        let scroll = NSScrollView::initWithFrame(
            mtm.alloc(),
            NSRect::new(area.origin, NSSize::new(area.size.width, list_height)),
        );
        scroll.setHasVerticalScroller(true);
        scroll.setBorderType(NSBorderType::BezelBorder);
        scroll.setDocumentView(Some(&table));

        // 窗口一打开是详情模式, 列表先藏着
        scroll.setHidden(true);
        echo.setHidden(true);

        Self {
            echo,
            scroll,
            table,
            rows,
        }
    }

    /// 把回显行与列表挂到窗口内容上.
    pub(super) fn attach(&self, content: &NSView) {
        content.addSubview(&self.echo);
        content.addSubview(&self.scroll);
    }

    /// 结果到了: 回显部件与命中数, 列表换成这一批字.
    pub(super) fn set_rows(&self, parts: &[char], found: &[char]) {
        self.fill(
            format!(
                "部件  {}      命中  {} 个字 (来源 zi.tools 字統网)",
                parts_text(parts),
                found.len()
            ),
            found.to_vec(),
        );
    }

    /// 等结果的时候先把列表露出来, 回显行显示「查询中」.
    pub(super) fn show_pending(&self, parts: &[char]) {
        self.fill(
            format!("部件  {}      查询中…", parts_text(parts)),
            Vec::new(),
        );
    }

    /// 请求失败或超时: 回显行说明结果, 列表清空.
    pub(super) fn show_failure(&self, parts: &[char], reason: &str) {
        self.fill(
            format!("部件  {}      {reason}", parts_text(parts)),
            Vec::new(),
        );
    }

    /// 换一批字并显出来; 上一批的选中行随即作废.
    fn fill(&self, echo: String, rows: Vec<char>) {
        self.echo.setStringValue(&NSString::from_str(&echo));
        self.rows.set_rows(rows);
        self.table.reloadData();
        // SAFETY: deselectAll: 的参数只是 sender, 可以给 nil
        unsafe { self.table.deselectAll(None) };
        self.reveal();
    }

    /// 藏回去, 把位置让给单字详情.
    pub(super) fn hide(&self) {
        self.scroll.setHidden(true);
        self.echo.setHidden(true);
    }

    fn reveal(&self) {
        self.scroll.setHidden(false);
        self.echo.setHidden(false);
    }

    /// 选中行里的字; 列表没显示或没选中时为 None.
    fn selected(&self) -> Option<char> {
        if self.scroll.isHidden() {
            return None;
        }
        self.rows.row(self.table.selectedRow())
    }

    /// 把选中的那个字复制到剪贴板, 返回它; 没选中时为 None.
    pub(super) fn copy_selected(&self) -> Option<char> {
        let zi = self.selected()?;
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        let text = NSString::from_str(&zi.to_string());
        // SAFETY: NSPasteboardTypeString 是系统给的只读类型标识
        let kind = unsafe { NSPasteboardTypeString };
        pasteboard.setString_forType(&text, kind);
        Some(zi)
    }
}

/// 部件回显: `木 木 木`.
fn parts_text(parts: &[char]) -> String {
    parts
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

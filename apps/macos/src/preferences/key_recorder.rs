use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSBezelStyle, NSButton, NSEvent, NSEventModifierFlags};
use objc2_foundation::{NSObjectProtocol, NSRect, NSString};
use qingjian_platform::{KeyCombo, MacModifier, MacSwitchKey, Modifiers};

use crate::imk::modifiers;

/// 录制中的按钮标题。
const RECORDING_TITLE: &str = "按下新的快捷键…";

/// Esc 的键码：取消录制。
const ESCAPE_KEY: u16 = 53;

/// 录什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderKind {
    /// 只记修饰键（配数字键上屏译词那两项）：按住修饰键再按任意键，键本身不算。
    ModifiersOnly,

    /// 修饰键 + 一个字母 / 数字（`control+option+t`）：没按修饰键的单键不算，继续等。
    Combo,

    /// 中 / 英切换键：修饰键单击（`left-command`，含左右）或修饰键 + 字母 / 数字的组合键。
    /// 单击的判定与运行时同一套：按下到抬起之间没插进别的键。
    SingleOrCombo,
}

/// 录制器的状态。
pub struct Ivars {
    /// 录什么。
    kind: RecorderKind,

    /// 正在等用户按键。
    recording: Cell<bool>,

    /// `SingleOrCombo` 用：切换键已按下，还没被别的键打断。
    held: Cell<Option<MacModifier>>,

    /// 当前值的配置写法（`control+option+t` / `shift+option` / `left-command`），`changed:` 里由 `setting_from_sender` 取走。
    recorded: RefCell<String>,

    /// 当前值给人看的写法，取消录制时恢复到标题上。
    label: RefCell<String>,
}

define_class!(
    // SAFETY: NSButton 允许子类化；没有实现 Drop。
    #[unsafe(super(NSButton))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    /// 快捷键录制按钮：点一下进入录制，按下组合键（或单击修饰键）就记下并发 `changed:`，Esc 或失焦取消。
    /// 标题显示当前值（`⌃⌥T` / `⇧⌥` / `左 ⌘`），配置写法放在 ivar 里。
    pub struct KeyRecorder;

    impl KeyRecorder {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        /// 点击：进入录制，成为第一响应者。不调父类，免得当成普通按钮点击发 action。
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            if let Some(window) = self.window() {
                window.makeFirstResponder(Some(self));
            }
            self.ivars().recording.set(true);
            self.ivars().held.set(None);
            self.setTitle(&NSString::from_str(RECORDING_TITLE));
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if !self.ivars().recording.get() {
                // SAFETY: 不在录制中就按普通按钮处理
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
                return;
            }
            if event.keyCode() == ESCAPE_KEY {
                self.cancel();
                return;
            }
            // 敲了普通键：单击判定作废（与运行时一致），这一键也不当切换键
            self.ivars().held.set(None);
            let flags = event.modifierFlags();
            let modifiers = Modifiers {
                option: flags.contains(NSEventModifierFlags::Option),
                shift: flags.contains(NSEventModifierFlags::Shift),
                control: flags.contains(NSEventModifierFlags::Control),
                command: flags.contains(NSEventModifierFlags::Command),
            };
            // 没按修饰键的单键不算快捷键，继续等
            if modifiers.is_empty() {
                return;
            }
            let value = if self.ivars().kind == RecorderKind::ModifiersOnly {
                Some((modifiers.key(), modifiers.label()))
            } else {
                event
                    .charactersIgnoringModifiers()
                    .and_then(|c| c.to_string().chars().next())
                    .filter(|c| c.is_ascii_alphanumeric())
                    .map(|c| {
                        let combo = KeyCombo {
                            modifiers,
                            key: c.to_ascii_lowercase(),
                        };
                        (combo.key_string(), combo.label())
                    })
            };
            let Some((key, label)) = value else {
                return;
            };
            self.finish(&key, &label);
        }

        /// 修饰键的按下抬起：中 / 英切换键可以配成一个修饰键单击（含左右）。
        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            if !self.ivars().recording.get() || self.ivars().kind != RecorderKind::SingleOrCombo {
                return;
            }
            let Some(modifier) = modifiers::modifier_key(event.keyCode()) else {
                return;
            };
            if event
                .modifierFlags()
                .contains(modifiers::modifier_flag(modifier))
            {
                self.ivars().held.set(Some(modifier));
                return;
            }
            if self.ivars().held.get() != Some(modifier) {
                return;
            }
            self.ivars().held.set(None);
            let key = MacSwitchKey::Modifier(modifier);
            self.finish(&key.key_string(), &key.label());
        }

        /// 失焦：录制没完成就恢复原标题。
        #[unsafe(method(resignFirstResponder))]
        fn resign_first_responder(&self) -> bool {
            if self.ivars().recording.get() {
                self.cancel();
            }
            // SAFETY: 父类的缺省实现
            unsafe { msg_send![super(self), resignFirstResponder] }
        }
    }

    unsafe impl NSObjectProtocol for KeyRecorder {}
);

impl KeyRecorder {
    /// `kind` 决定录成什么（见 [`RecorderKind`]）。
    pub fn new(mtm: MainThreadMarker, kind: RecorderKind) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(Ivars {
            kind,
            recording: Cell::new(false),
            held: Cell::new(None),
            recorded: RefCell::new(String::new()),
            label: RefCell::new(String::new()),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setBezelStyle(NSBezelStyle::Push);
        this
    }

    /// 按配置刷新显示：`key` 是配置写法，`label` 是给人看的。
    pub fn show(&self, key: &str, label: &str) {
        if self.ivars().recording.get() {
            return;
        }
        *self.ivars().recorded.borrow_mut() = key.to_owned();
        *self.ivars().label.borrow_mut() = label.to_owned();
        self.setTitle(&NSString::from_str(label));
    }

    /// 最近一次录到的值的配置写法。
    pub fn recorded(&self) -> String {
        self.ivars().recorded.borrow().clone()
    }

    /// 录到了一条：记下、改标题、交回第一响应者、发 `changed:`。
    fn finish(&self, key: &str, label: &str) {
        self.ivars().recording.set(false);
        self.ivars().held.set(None);
        *self.ivars().recorded.borrow_mut() = key.to_owned();
        *self.ivars().label.borrow_mut() = label.to_owned();
        self.setTitle(&NSString::from_str(label));
        if let Some(window) = self.window() {
            window.makeFirstResponder(None);
        }
        // target / action 由 `wire` 挂上，与其他控件一致
        let target: Option<Retained<AnyObject>> = self.target();
        let action = self.action();
        // SAFETY: 选择器是 PreferencesTarget 上定义的 `changed:`，签名 (id) -> void
        unsafe {
            self.sendAction_to(action, target.as_deref());
        }
    }

    fn cancel(&self) {
        self.ivars().recording.set(false);
        self.ivars().held.set(None);
        let label = self.ivars().label.borrow().clone();
        self.setTitle(&NSString::from_str(&label));
    }
}

pub mod app;
pub mod backend;
pub mod bar;
pub mod cli;
pub mod config;
pub mod dispatch;
pub mod input;
pub mod ipc;
pub mod layout;
pub mod protocols;
pub mod process;
pub mod state;
pub mod sync;

pub use app::App;
pub use backend::{
    collect_render_elements, CursorManager, OutputInfo, OutputManager, RenderManager,
    TrussRenderElement, TtyBackend, DESKTOP_BG_COLOR,
};
pub use bar::run_status_bar;
pub use cli::{handle_msg_command, CliArgs, Subcommand};
pub use config::LuaConfig;
pub use dispatch::{Command, Direction, DispatchError, DispatchResult, Dispatcher, Event};
pub use input::{
    KeyAction, KeyPattern, Keybindings, Modifiers, PointerDragMode, PointerFocusTarget,
    PointerState,
};
pub use ipc::{IpcRequest, IpcResponse, IpcServer};
pub use layout::{
    CustomLayout, GridLayout, Layout, LayoutConfig, LayoutRegistry, MasterLayout, MonocleLayout,
};
pub use state::{
    Rect, State, StateError, Window, WindowId, WindowRule, WindowRuleAction, WindowRuleManager,
    WindowRuleMatcher, Workspace,
};
pub use sync::{Transaction, TransactionManager, TRANSACTION_TIMEOUT};

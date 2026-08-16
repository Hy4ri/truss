pub mod app;
pub mod backend;
pub mod dispatch;
pub mod input;
pub mod ipc;
pub mod layout;
pub mod protocols;
pub mod state;

pub use app::App;
pub use backend::{OutputManager, RenderManager, DESKTOP_BG_COLOR};
pub use dispatch::{Command, Direction, DispatchError, DispatchResult, Dispatcher, Event};
pub use input::{KeyAction, KeyPattern, Keybindings, Modifiers, PointerFocusTarget, PointerState};
pub use ipc::{IpcRequest, IpcResponse, IpcServer};
pub use layout::{Layout, LayoutConfig, LayoutRegistry, MasterLayout, MonocleLayout};
pub use state::{Rect, State, StateError, Window, WindowId, Workspace};

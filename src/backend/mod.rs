pub mod drm;
pub mod output;
pub mod render;
pub mod renderer;
pub mod tty;

pub use drm::{discover_and_init_drm_displays, DrmDisplay};
pub use output::{OutputInfo, OutputManager};
pub use render::collect_render_elements;
pub use renderer::{RenderManager, DESKTOP_BG_COLOR};
pub use tty::TtyBackend;

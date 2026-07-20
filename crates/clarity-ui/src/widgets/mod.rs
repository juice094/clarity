//! Reusable egui widgets that depend only on the theme and design system.

pub mod async_state;
pub mod avatar;
pub mod button;
pub mod collapsible_section;
pub mod icon_button;
pub mod interactive_row;
pub mod modal;
pub mod nav_icon_rail;
pub mod nav_row;
pub mod overlay;
pub mod provider_row;
pub mod status_message;
pub mod text_input;
pub mod theme_card;
pub mod user_avatar;
pub mod window_control;

pub use button::{
    Button, ButtonSize, ButtonVariant, ghost_button, primary_button, secondary_button,
};
pub use icon_button::{icon_button, icon_button_toolbar, icon_button_toolbar_colored};
pub use interactive_row::interactive_row;
pub use modal::{Modal, ModalAnchor, modal_scrim};
pub use nav_icon_rail::{nav_icon_rail, nav_status_dot};
pub use nav_row::{nav_row, nav_row_with_trailing};
pub use overlay::{Overlay, OverlayAnchor, overlay_scrim};
pub use provider_row::provider_row;
pub use text_input::TextInput;
pub use theme_card::theme_card;
pub use user_avatar::user_avatar_row;
pub use window_control::window_control_button;

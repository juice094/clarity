pub mod avatar;
pub mod collapsible_section;
pub mod command_palette;
pub mod draft_indicator;
pub mod icon_button;
pub mod interactive_row;
pub mod pretext_probe;
pub mod provider_row;
pub mod rich_paragraph;
pub mod status_message;
pub mod theme_card;
pub mod user_avatar;
pub mod window_control;

// pub use icon_button::icon_button; // unused — prefer icon_button_toolbar
// pub use icon_button::icon_button_primary; // restored via git history if needed
pub use icon_button::{icon_button, icon_button_toolbar};
pub use interactive_row::interactive_row;
pub use provider_row::provider_row;
pub use theme_card::theme_card;
pub use user_avatar::user_avatar_row;
pub use window_control::window_control_button;

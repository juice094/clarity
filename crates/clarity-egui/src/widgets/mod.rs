pub mod badge;
pub mod card;
pub mod command_palette;
pub mod icon_button;
pub mod interactive_row;
pub mod provider_row;
pub mod settings_row;
pub mod sidebar_card;
pub mod status_capsule;
pub mod status_dot;
pub mod tab_button;
pub mod theme_card;
pub mod toggle;
pub mod window_control;

// pub use icon_button::icon_button; // unused — prefer icon_button_toolbar
// pub use icon_button::icon_button_primary; // restored via git history if needed
pub use icon_button::icon_button_toolbar;
pub use interactive_row::interactive_row;
pub use provider_row::provider_row;
// pub use settings_row::settings_row; // not yet integrated into tabs
pub use sidebar_card::sidebar_card;
pub use status_capsule::status_capsule;
pub use status_dot::status_dot;
pub use tab_button::tab_button;
pub use theme_card::theme_card;
// pub use toggle::toggle; // not yet used in UI
pub use window_control::window_control_button;

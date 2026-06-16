pub mod avatar;
pub mod command_palette;
pub mod icon_button;
pub mod interactive_row;
pub mod persona_switcher;
pub mod pretext_probe;
pub mod provider_row;
pub mod rich_paragraph;
pub mod sidebar_card;
pub mod status_capsule;
pub mod tab_button;
pub mod theme_card;
pub mod user_avatar;
pub mod window_control;

// pub use icon_button::icon_button; // unused — prefer icon_button_toolbar
// pub use icon_button::icon_button_primary; // restored via git history if needed
pub use icon_button::{icon_button, icon_button_toolbar};
pub use interactive_row::interactive_row;
pub use persona_switcher::persona_switcher;
// `PersonaSwitcherResponse` is accessed via the fully-qualified path inside
// `main::render_persona_switcher`; not re-exported here to avoid unused warnings.
pub use provider_row::provider_row;
pub use sidebar_card::sidebar_card;
pub use status_capsule::status_capsule;
pub use tab_button::tab_button;
pub use theme_card::theme_card;
pub use user_avatar::user_avatar_row;
pub use window_control::window_control_button;

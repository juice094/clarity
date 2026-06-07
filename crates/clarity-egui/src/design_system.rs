//! Semantic design-system layer over raw egui APIs.
//!
//! Solves three problems with raw egui:
//!   1. Style scattering — Frame::new().fill().stroke().corner_radius().inner_margin()
//!      repeated in every panel.
//!   2. Layout magic — egui::Layout::left_to_right(egui::Align::Center) is
//!      error-prone and non-semantic.
//!   3. No reuse — every panel re-invents its own visual treatment.
//!
//! This module injects Theme into egui::Context::data(), so all helpers are
//! zero-parameter after a single `install_theme()` call at app startup.
//!
//! Usage (after theme install):
//!
//!   row(ui, |ui| {
//!       gap(ui, Space::S2);
//!       surface(ui, Surface::Card, |ui| {
//!           text(ui, "Hello", Text::Body);
//!       });
//!   });

use crate::theme::Theme;

// =============================================================================
// Theme injection — store Theme in egui Context so helpers can auto-retrieve
// =============================================================================

fn theme_id() -> egui::Id {
    egui::Id::new("clarity_design_system_theme")
}

/// Install Theme into the egui Context. Call once per frame (or when theme changes).
pub fn install_theme(ctx: &egui::Context, theme: Theme) {
    ctx.data_mut(|d| d.insert_temp(theme_id(), theme));
}

/// Retrieve the installed Theme from Context. Panics if not installed.
fn theme(ctx: &egui::Context) -> Theme {
    ctx.data(|d| d.get_temp::<Theme>(theme_id()))
        .expect("Theme not installed — call install_theme() at app startup")
}

// =============================================================================
// Surface — visual layering (background, border, radius, shadow, padding)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Primary app background — no border, no radius, no padding.
    Canvas,
    /// Elevated card with border, radius, shadow, padding.
    Card,
    /// Nested content area — tinted bg, medium radius, no border.
    Nested,
    /// Approval/danger highlight — strong border radius, accent shadow.
    Prompt,
    /// Inline error block — tinted red bg.
    Error,
    /// Subtle hoverable row — transparent bg, hover tint.
    Row,
    /// Code block / monospace content area.
    Code,
    /// Input field background.
    Input,
}

impl Surface {
    fn frame(self, t: &Theme) -> egui::Frame {
        match self {
            Surface::Canvas => egui::Frame::new().fill(t.bg),
            Surface::Card => egui::Frame::new()
                .fill(t.bg)
                .stroke(egui::Stroke::new(0.5, t.border))
                .corner_radius(egui::CornerRadius::same(t.radius_lg.round() as u8))
                .shadow(t.shadow_card),
            Surface::Nested => egui::Frame::new()
                .fill(t.surface)
                .corner_radius(egui::CornerRadius::same(t.radius_md.round() as u8)),
            Surface::Prompt => egui::Frame::new()
                .fill(t.bg_accent)
                .stroke(egui::Stroke::new(0.5, t.border))
                .corner_radius(egui::CornerRadius::same(t.radius_xl.round() as u8))
                .shadow(t.shadow_panel),
            Surface::Error => egui::Frame::new()
                .fill(t.error_bubble)
                .corner_radius(egui::CornerRadius::same(t.radius_md.round() as u8)),
            Surface::Row => egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .corner_radius(egui::CornerRadius::same(t.radius_sm.round() as u8)),
            Surface::Code => egui::Frame::new()
                .fill(t.code_block_bg)
                .corner_radius(egui::CornerRadius::same(t.radius_sm.round() as u8)),
            Surface::Input => egui::Frame::new()
                .fill(t.input_bg)
                .stroke(egui::Stroke::new(0.5, t.border))
                .corner_radius(egui::CornerRadius::same(t.radius_md.round() as u8)),
        }
    }

    fn padding(self, t: &Theme) -> egui::Margin {
        match self {
            Surface::Canvas | Surface::Row => egui::Margin::ZERO,
            Surface::Card | Surface::Prompt => {
                egui::Margin::same(t.space_16.round() as i8)
            }
            Surface::Nested | Surface::Error | Surface::Code | Surface::Input => {
                egui::Margin::symmetric(t.space_12.round() as i8, t.space_12.round() as i8)
            }
        }
    }
}

/// Render a semantic surface. Theme is retrieved from Context automatically.
pub fn surface<R>(
    ui: &mut egui::Ui,
    kind: Surface,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Response, R) {
    let t = theme(ui.ctx());
    let mut frame = kind.frame(&t);
    frame = frame.inner_margin(kind.padding(&t));
    let inner = frame.show(ui, add_contents);
    (inner.response, inner.inner)
}

// =============================================================================
// Stack — layout primitives (row, column, center, space-between)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

impl HAlign {
    fn to_egui(self) -> egui::Align {
        match self {
            HAlign::Left => egui::Align::Min,
            HAlign::Center => egui::Align::Center,
            HAlign::Right => egui::Align::Max,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

impl VAlign {
    fn to_egui(self) -> egui::Align {
        match self {
            VAlign::Top => egui::Align::Min,
            VAlign::Center => egui::Align::Center,
            VAlign::Bottom => egui::Align::Max,
        }
    }
}

/// Horizontal row, items left-to-right, vertically centered.
pub fn row<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| add_contents(ui)).inner
}

/// Horizontal row with explicit cross-axis alignment.
pub fn row_align<R>(
    ui: &mut egui::Ui,
    v: VAlign,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.with_layout(
        egui::Layout::left_to_right(v.to_egui()),
        |ui| add_contents(ui),
    )
    .inner
}

/// Vertical column, items top-to-bottom, horizontally left-aligned.
pub fn col<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.vertical(|ui| add_contents(ui)).inner
}

/// Vertical column with explicit cross-axis alignment.
pub fn col_align<R>(
    ui: &mut egui::Ui,
    h: HAlign,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.with_layout(
        egui::Layout::top_down(h.to_egui()),
        |ui| add_contents(ui),
    )
    .inner
}

/// Center content in both axes.
pub fn center<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.with_layout(
        egui::Layout::top_down_justified(egui::Align::Center),
        |ui| add_contents(ui),
    )
    .inner
}

/// Push next item to the right edge.
pub fn push_right(ui: &mut egui::Ui) {
    ui.add_space(ui.available_width());
}

/// Push next item to the bottom edge.
pub fn push_bottom(ui: &mut egui::Ui) {
    ui.add_space(ui.available_height());
}

/// Right-align a block of content.
pub fn right<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| add_contents(ui))
        .inner
}

/// Constrain width to a fraction of available space.
pub fn width_pct(ui: &mut egui::Ui, fraction: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    let max_w = ui.available_width() * fraction.clamp(0.0, 1.0);
    ui.set_max_width(max_w);
    add_contents(ui);
}

// =============================================================================
// Text — typography semantics
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Text {
    /// 18px, strong, primary color.
    Title,
    /// 16px, strong, primary color.
    Headline,
    /// 14px, normal, primary color.
    Body,
    /// 14px, strong, primary color.
    BodyStrong,
    /// 14px, normal, muted color.
    BodyMuted,
    /// 12px, normal, dim color.
    Caption,
    /// 10px, normal, dim color.
    Small,
    /// 13px, mono, secondary color.
    Code,
    /// 14px, normal, danger color.
    Error,
    /// 14px, strong, accent color.
    Accent,
    /// 14px, normal, dim color.
    Placeholder,
}

impl Text {
    fn size(self, t: &Theme) -> f32 {
        match self {
            Text::Title => t.text_xl,
            Text::Headline => t.text_lg,
            Text::Body | Text::BodyStrong | Text::BodyMuted | Text::Error | Text::Accent
            | Text::Placeholder => t.text_base,
            Text::Caption => t.text_sm,
            Text::Small => t.text_xs,
            Text::Code => t.text_sm,
        }
    }

    fn color(self, t: &Theme) -> egui::Color32 {
        match self {
            Text::Title | Text::Headline | Text::Body | Text::BodyStrong => t.text,
            Text::BodyMuted => t.text_muted,
            Text::Caption | Text::Small | Text::Placeholder => t.text_dim,
            Text::Code => t.text_muted,
            Text::Error => t.danger,
            Text::Accent => t.accent,
        }
    }

    fn strong(self) -> bool {
        matches!(self, Text::Title | Text::Headline | Text::BodyStrong | Text::Accent)
    }

    fn mono(self) -> bool {
        matches!(self, Text::Code)
    }

    fn to_richtext(self, t: &Theme, content: impl Into<String>) -> egui::RichText {
        let mut rt = egui::RichText::new(content.into())
            .size(self.size(t))
            .color(self.color(t));
        if self.strong() {
            rt = rt.strong();
        }
        if self.mono() {
            rt = rt.monospace();
        }
        rt
    }
}

/// Render semantic text. Theme auto-retrieved from Context.
pub fn text(ui: &mut egui::Ui, content: impl Into<String>, style: Text) -> egui::Response {
    let t = theme(ui.ctx());
    ui.label(style.to_richtext(&t, content))
}

/// Render a headline followed by a standard gap.
pub fn heading(ui: &mut egui::Ui, content: impl Into<String>) {
    text(ui, content, Text::Headline);
    gap(ui, Space::S2);
}

// =============================================================================
// Spacer — spacing tokens (4px baseline)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// 4px — tight inset, icon gaps.
    S0,
    /// 8px — default element gap.
    S1,
    /// 12px — related group separation.
    S2,
    /// 16px — card padding, section gap.
    S3,
    /// 20px — medium section gap.
    S4,
    /// 24px — large block separator.
    S5,
    /// 40px — page-level spacing.
    S6,
}

impl Space {
    fn px(self, t: &Theme) -> f32 {
        match self {
            Space::S0 => t.space_4,
            Space::S1 => t.space_8,
            Space::S2 => t.space_12,
            Space::S3 => t.space_16,
            Space::S4 => t.space_20,
            Space::S5 => t.space_24,
            Space::S6 => t.space_40,
        }
    }
}

/// Add semantic spacing. Theme auto-retrieved from Context.
pub fn gap(ui: &mut egui::Ui, space: Space) {
    let t = theme(ui.ctx());
    ui.add_space(space.px(&t));
}

/// Horizontal divider line.
pub fn divider(ui: &mut egui::Ui) {
    let t = theme(ui.ctx());
    let y = ui.cursor().min.y;
    let x_range = ui.available_rect_before_wrap().x_range();
    ui.add(egui::Separator::default().spacing(0.0));
    ui.painter().hline(x_range, y, egui::Stroke::new(0.5, t.border));
}

// =============================================================================
// Composable patterns
// =============================================================================

/// Key-value row: label left, value right.
pub fn key_value(
    ui: &mut egui::Ui,
    key: impl Into<String>,
    value: impl Into<String>,
    key_style: Text,
    value_style: Text,
) {
    row(ui, |ui| {
        text(ui, key, key_style);
        push_right(ui);
        text(ui, value, value_style);
    });
}

/// List item with hover background and optional trailing action.
pub fn list_item<R>(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    trailing: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Response, R) {
    let t = theme(ui.ctx());
    let mut frame = Surface::Row.frame(&t);
    frame = frame.inner_margin(Surface::Row.padding(&t));
    let resp = frame.show(ui, |ui| {
        row(ui, |ui| {
            text(ui, label, Text::Body);
            push_right(ui);
            trailing(ui)
        })
    });
    (resp.response, resp.inner)
}

/// Icon + label row.
pub fn icon_text(ui: &mut egui::Ui, icon: &str, label: impl Into<String>, style: Text) {
    row(ui, |ui| {
        text(ui, icon, Text::BodyMuted);
        gap(ui, Space::S1);
        text(ui, label, style);
    });
}

// =============================================================================
// Button primitives
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Secondary,
    Danger,
    Ghost,
}

/// Semantic button. Theme auto-retrieved.
pub fn btn(ui: &mut egui::Ui, label: impl Into<String>, style: ButtonStyle) -> egui::Response {
    let t = theme(ui.ctx());
    let (bg, fg, radius) = match style {
        ButtonStyle::Primary => (t.accent, t.bg, t.radius_md),
        ButtonStyle::Secondary => (t.surface, t.text, t.radius_sm),
        ButtonStyle::Danger => (t.danger, t.bg, t.radius_md),
        ButtonStyle::Ghost => (egui::Color32::TRANSPARENT, t.text, t.radius_sm),
    };
    ui.add(
        egui::Button::new(
            egui::RichText::new(label.into())
                .size(t.text_base)
                .color(fg),
        )
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(radius.round() as u8)),
    )
}

/// Small toolbar icon button.
pub fn btn_icon(ui: &mut egui::Ui, icon: &str) -> egui::Response {
    let t = theme(ui.ctx());
    ui.add(
        egui::Button::new(
            egui::RichText::new(icon)
                .size(t.text_sm)
                .color(t.text_dim),
        )
        .fill(egui::Color32::TRANSPARENT)
        .corner_radius(egui::CornerRadius::same(t.radius_sm.round() as u8)),
    )
}

// =============================================================================
// Scroll — semantic scroll areas (eliminates ScrollArea boilerplate)
// =============================================================================

/// Semantic scroll configurations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scroll {
    /// Vertical scroll, auto-shrinks to content.
    Vertical,
    /// Vertical scroll with a max height (most common for lists).
    VerticalMax(f32),
    /// Horizontal scroll.
    Horizontal,
    /// Both directions.
    Both,
}

/// Render a semantic scroll area. Theme auto-retrieved.
///
/// ```rust
/// scroll(ui, Scroll::VerticalMax(200.0), |ui| {
///     for item in items {
///         text(ui, item, Text::Body);
///     }
/// });
/// ```
pub fn scroll<R>(
    ui: &mut egui::Ui,
    kind: Scroll,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let t = theme(ui.ctx());
    match kind {
        Scroll::Vertical => {
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, add_contents)
                .inner
        }
        Scroll::VerticalMax(max_h) => {
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .auto_shrink([false, true])
                .show(ui, add_contents)
                .inner
        }
        Scroll::Horizontal => {
            egui::ScrollArea::horizontal()
                .auto_shrink([true, false])
                .show(ui, add_contents)
                .inner
        }
        Scroll::Both => {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, add_contents)
                .inner
        }
    }
}

/// Scroll area with custom scrollbar styling (Kimi-style: thin, themed thumb).
pub fn scroll_styled<R>(
    ui: &mut egui::Ui,
    kind: Scroll,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let t = theme(ui.ctx());
    let scroll_id = ui.id().with("scroll_styled");
    let mut area = match kind {
        Scroll::Vertical => egui::ScrollArea::vertical(),
        Scroll::VerticalMax(max_h) => egui::ScrollArea::vertical().max_height(max_h),
        Scroll::Horizontal => egui::ScrollArea::horizontal(),
        Scroll::Both => egui::ScrollArea::both(),
    };
    area = area.id_salt(scroll_id);
    area = area.auto_shrink([false, true]);
    // NOTE: egui 0.31 does not expose per-ScrollArea scrollbar color APIs.
    // The theme's scrollbar appearance is controlled globally via style override.
    area.show(ui, add_contents).inner
}

// =============================================================================
// Modal — semantic modal content (frame + padding + width constraints)
// =============================================================================

/// Modal content specification.
pub struct ModalSpec {
    pub title: String,
    pub min_width: f32,
    pub max_width: f32,
}

impl ModalSpec {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            min_width: 420.0,
            max_width: 600.0,
        }
    }
    pub fn width(mut self, w: f32) -> Self {
        self.min_width = w;
        self.max_width = w;
        self
    }
    pub fn width_range(mut self, min: f32, max: f32) -> Self {
        self.min_width = min;
        self.max_width = max;
        self
    }
}

/// Render a full modal with backdrop blocker + centered window.
///
/// Returns the inner content result if the window is shown.
///
/// ```rust
/// modal(ctx, ModalSpec::new("Confirm"), |ui| {
///     text(ui, "Are you sure?", Text::Body);
///     gap(ui, Space::S3);
///     right(ui, |ui| {
///         btn(ui, "Cancel", ButtonStyle::Ghost);
///         btn(ui, "OK", ButtonStyle::Primary);
///     });
/// });
/// ```
pub fn modal<R>(
    ctx: &egui::Context,
    spec: ModalSpec,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    let t = theme(ctx);
    let screen = ctx.screen_rect();

    // Backdrop blocker
    let blocker_id = egui::Id::new(("modal_blocker", &spec.title));
    egui::Area::new(blocker_id)
        .order(egui::Order::Background)
        .interactable(true)
        .show(ctx, |ui| {
            let resp = ui.allocate_response(screen.size(), egui::Sense::click());
            ui.painter_at(resp.rect)
                .rect_filled(resp.rect, 0.0, t.overlay);
        });

    // Modal window
    egui::Window::new(&spec.title)
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::group(&ctx.style())
                .fill(t.surface)
                .corner_radius(egui::CornerRadius::same(t.radius_md.round() as u8))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::same(20)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(spec.min_width);
            ui.set_max_width(spec.max_width);
            add_contents(ui)
        })
        .and_then(|r| r.inner)
}

/// Render only the modal backdrop blocker (use when you need a custom Window).
pub fn modal_blocker(ctx: &egui::Context) {
    let t = theme(ctx);
    let screen = ctx.screen_rect();
    let blocker_id = egui::Id::new("modal_blocker_generic");
    egui::Area::new(blocker_id)
        .order(egui::Order::Background)
        .interactable(true)
        .show(ctx, |ui| {
            let resp = ui.allocate_response(screen.size(), egui::Sense::click());
            ui.painter_at(resp.rect)
                .rect_filled(resp.rect, 0.0, t.overlay);
        });
}

// =============================================================================
// Tabs — semantic tab bar
// =============================================================================

/// Tab descriptor for semantic tab bars.
pub struct TabItem {
    pub label: String,
    pub active: bool,
}

impl TabItem {
    pub fn new(label: impl Into<String>, active: bool) -> Self {
        Self {
            label: label.into(),
            active,
        }
    }
}

/// Render a semantic tab bar. Returns the index of the clicked tab (if any).
///
/// ```rust
/// let tabs = vec![
///     TabItem::new("Chat", true),
///     TabItem::new("Settings", false),
/// ];
/// if let Some(idx) = tab_bar(ui, &tabs) {
///     active_tab = idx;
/// }
/// ```
pub fn tab_bar(ui: &mut egui::Ui, tabs: &[TabItem]) -> Option<usize> {
    let t = theme(ui.ctx());
    let mut clicked: Option<usize> = None;

    row(ui, |ui| {
        ui.spacing_mut().item_spacing.x = t.space_4;
        for (i, tab) in tabs.iter().enumerate() {
            let (bg, fg, weight) = if tab.active {
                (t.surface, t.text, true)
            } else {
                (egui::Color32::TRANSPARENT, t.text_dim, false)
            };
            let mut rt = egui::RichText::new(&tab.label)
                .size(t.text_base)
                .color(fg);
            if weight {
                rt = rt.strong();
            }
            let resp = ui.add(
                egui::Button::new(rt)
                    .fill(bg)
                    .corner_radius(egui::CornerRadius::same(t.radius_sm.round() as u8)),
            );
            if resp.clicked() && !tab.active {
                clicked = Some(i);
            }
        }
    });

    // Active tab underline
    if let Some(active_idx) = tabs.iter().position(|t| t.active) {
        let tab_count = tabs.len();
        let total_spacing = if tab_count > 1 {
            (tab_count - 1) as f32 * t.space_4
        } else {
            0.0
        };
        let tab_w = (ui.available_width() - total_spacing) / tab_count.max(1) as f32;
        let line_x = ui.min_rect().min.x + active_idx as f32 * (tab_w + t.space_4);
        let line_y = ui.cursor().min.y - 2.0;
        ui.painter().hline(
            line_x..=line_x + tab_w,
            line_y,
            egui::Stroke::new(2.0, t.accent),
        );
    }

    clicked
}

// =============================================================================
// Form — input primitives with semantic styling
// =============================================================================

/// Single-line text input with semantic styling.
///
/// ```rust
/// let mut value = String::new();
/// input(ui, &mut value, "Enter name…");
/// ```
pub fn input(
    ui: &mut egui::Ui,
    value: &mut String,
    hint: impl Into<String>,
) -> egui::Response {
    let t = theme(ui.ctx());
    let hint_str: String = hint.into();
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint_str)
            .font(egui::FontId::proportional(t.text_base))
            .margin(egui::vec2(t.space_8, t.space_4)),
    )
}

/// Multi-line text input with semantic styling.
pub fn input_multiline(
    ui: &mut egui::Ui,
    value: &mut String,
    hint: impl Into<String>,
) -> egui::Response {
    let t = theme(ui.ctx());
    let hint_str: String = hint.into();
    ui.add(
        egui::TextEdit::multiline(value)
            .hint_text(hint_str)
            .font(egui::FontId::proportional(t.text_base))
            .margin(egui::vec2(t.space_8, t.space_4)),
    )
}

/// Labeled input group: label above, input below.
pub fn field(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    value: &mut String,
    hint: impl Into<String>,
) -> egui::Response {
    col(ui, |ui| {
        text(ui, label, Text::Caption);
        gap(ui, Space::S0);
        input(ui, value, hint)
    })
}

/// Checkbox with semantic label styling.
pub fn checkbox(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    value: &mut bool,
) -> egui::Response {
    ui.checkbox(value, label.into())
}

/// Select dropdown with semantic styling.
/// Returns true if selection changed.
pub fn select(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    options: &[String],
    selected: &mut usize,
) -> bool {
    let mut changed = false;
    col(ui, |ui| {
        text(ui, label, Text::Caption);
        gap(ui, Space::S0);
        egui::ComboBox::from_id_salt(ui.id().with("select"))
            .selected_text(options.get(*selected).map(|s| s.as_str()).unwrap_or(""))
            .show_ui(ui, |ui| {
                for (i, opt) in options.iter().enumerate() {
                    if ui.selectable_label(i == *selected, opt).clicked() {
                        *selected = i;
                        changed = true;
                    }
                }
            });
    });
    changed
}

/// A form section with a headline and standard padding.
pub fn form_section(ui: &mut egui::Ui, title: impl Into<String>, add_contents: impl FnOnce(&mut egui::Ui)) {
    col(ui, |ui| {
        heading(ui, title);
        gap(ui, Space::S1);
        add_contents(ui);
    });
}

// =============================================================================
// Status indicators
// =============================================================================

/// Status dot with semantic color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Online,
    Busy,
    Offline,
    Error,
    Warning,
}

impl Status {
    fn color(self, t: &Theme) -> egui::Color32 {
        match self {
            Status::Online => t.ok,
            Status::Busy => t.status_busy,
            Status::Offline => t.text_dim,
            Status::Error => t.danger,
            Status::Warning => t.warn,
        }
    }
}

/// Render a small status dot.
pub fn status_dot(ui: &mut egui::Ui, status: Status) -> egui::Response {
    let t = theme(ui.ctx());
    let size = egui::vec2(8.0, 8.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, status.color(&t));
    response
}

/// Render a status badge (colored pill with text).
pub fn status_badge(ui: &mut egui::Ui, label: impl Into<String>, status: Status) -> egui::Response {
    let t = theme(ui.ctx());
    let color = status.color(&t);
    // Create a subtle background tint
    let bg = egui::Color32::from_rgba_premultiplied(
        color.r(),
        color.g(),
        color.b(),
        30,
    );
    let frame = egui::Frame::new()
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(t.radius_full.round() as u8))
        .inner_margin(egui::Margin::symmetric(t.space_8.round() as i8, t.space_4.round() as i8));
    frame.show(ui, |ui| {
        row(ui, |ui| {
            status_dot(ui, status);
            gap(ui, Space::S0);
            let mut rt = egui::RichText::new(label.into()).size(t.text_sm);
            rt = rt.color(color);
            ui.label(rt)
        })
    })
    .inner
}

// =============================================================================
// Responsive utilities
// =============================================================================

/// Render different content based on available width.
///
/// ```rust
/// responsive(ui, |ui, size| {
///     match size {
///         Responsive::Compact => text(ui, "Compact view", Text::Body),
///         Responsive::Medium  => text(ui, "Medium view", Text::Body),
///         Responsive::Wide    => text(ui, "Wide view", Text::Body),
///     }
/// });
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Responsive {
    Compact,
    Medium,
    Wide,
}

pub fn responsive<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui, Responsive) -> R,
) -> R {
    let t = theme(ui.ctx());
    let available = ui.available_width();
    let size = if available < t.breakpoint_compact {
        Responsive::Compact
    } else if available < t.breakpoint_medium {
        Responsive::Medium
    } else {
        Responsive::Wide
    };
    add_contents(ui, size)
}

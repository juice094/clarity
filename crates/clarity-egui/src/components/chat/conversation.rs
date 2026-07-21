//! Kimi-style conversation UI components — consolidated single file.
//!
//! Extracted from Kimi Desktop v3.0.15 `ConversationView` CSS/JS chunks.
//! Provides: thinking blocks, tool groups, subagent trackers, approval dock,
//! and knowledge panel.

use crate::theme::Theme;
use crate::ui::types::{ToolCallInfo, ToolCallStatus};

// ============================================================================
// Primitives
// ============================================================================

/// card style variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CardStyle {
    Approval,
    ListItem,
    Nested,
    Error,
}

impl CardStyle {
    fn frame(self, theme: &Theme) -> egui::Frame {
        match self {
            CardStyle::Approval => clarity_ui::design_system::Elevation::Elevated
                .frame(theme)
                .fill(theme.bg_accent)
                .stroke(egui::Stroke::new(0.5, theme.border))
                // LAYOUT-EXEMPT: 20px approval-card radius sits between
                // radius_md (16) and radius_lg (28).
                .corner_radius(egui::CornerRadius::same(20))
                .shadow(theme.shadow_card),
            CardStyle::ListItem => clarity_ui::design_system::Elevation::Base
                .frame(theme)
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(0.5, theme.border))
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8)),
            CardStyle::Nested => clarity_ui::design_system::Elevation::Surface
                .frame(theme)
                .fill(theme.surface)
                // LAYOUT-EXEMPT: 12px nested-card radius sits between
                // radius_sm (8) and radius_md (16).
                .corner_radius(egui::CornerRadius::same(12)),
            CardStyle::Error => clarity_ui::design_system::Elevation::Elevated
                .frame(theme)
                .fill(theme.error_bubble)
                // LAYOUT-EXEMPT: 10px error-card radius sits between
                // radius_sm (8) and radius_md (16).
                .corner_radius(egui::CornerRadius::same(10)),
        }
    }

    fn inner_margin(self) -> egui::Margin {
        match self {
            CardStyle::Approval | CardStyle::ListItem => egui::Margin::same(16),
            CardStyle::Nested | CardStyle::Error => egui::Margin::symmetric(12, 10),
        }
    }
}

/// Renders a styled card container.
pub fn card<R>(
    ui: &mut egui::Ui,
    theme: &Theme,
    style: CardStyle,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Response, R) {
    let mut frame = style.frame(theme);
    frame = frame.inner_margin(style.inner_margin());
    let is_list_item = style == CardStyle::ListItem;
    let mut resp = None;

    let inner = frame.show(ui, |ui| {
        if is_list_item {
            let item_resp = ui.interact(
                ui.max_rect(),
                ui.id().with("card_hover"),
                egui::Sense::hover(),
            );
            if item_resp.hovered() {
                ui.painter().rect_filled(
                    item_resp.rect,
                    egui::CornerRadius::same(theme.radius_md as u8),
                    theme.bg_hover,
                );
            }
            resp = Some(item_resp);
        }
        add_contents(ui)
    });

    let response = resp.unwrap_or_else(|| {
        ui.interact(
            inner.response.rect,
            ui.id().with("card"),
            egui::Sense::hover(),
        )
    });
    (response, inner.inner)
}

/// Holds collapsible state.
#[derive(Clone, Copy, Debug, Default)]
pub struct CollapsibleState {
    pub expanded: bool,
}

/// Holds collapsible header state.
pub struct CollapsibleHeader<'a> {
    pub label: &'a str,
    pub is_loading: bool,
    pub icon: Option<&'a str>,
}

impl<'a> CollapsibleHeader<'a> {
    /// Creates a new instance.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            is_loading: false,
            icon: None,
        }
    }
    /// Loading.
    #[allow(dead_code)]
    pub fn loading(mut self, v: bool) -> Self {
        self.is_loading = v;
        self
    }
    /// Icon.
    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Renders a collapsible header/body section.
pub fn collapsible<R>(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: egui::Id,
    header: CollapsibleHeader<'_>,
    state: &mut CollapsibleState,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> bool {
    ui.vertical(|ui| {
        let header_resp = ui.horizontal(|ui| {
            ui.set_min_height(32.0);
            let chevron = if state.expanded {
                crate::theme::ICON_CARET_DOWN
            } else {
                crate::theme::ICON_CARET_RIGHT
            };
            let chevron_text = egui::RichText::new(chevron)
                .font(theme.font_icon(16.0))
                .color(theme.text_dim);
            let chevron_resp = ui.add(egui::Label::new(chevron_text).sense(egui::Sense::click()));

            if let Some(icon) = header.icon {
                ui.label(
                    egui::RichText::new(icon)
                        .font(theme.font_icon(16.0))
                        .color(theme.text_muted),
                );
            }

            let label_color = if header.is_loading {
                let t = ui.ctx().input(|i| i.time as f32);
                let phase = (t * 2.0).sin() * 0.5 + 0.5;
                egui::Color32::from_rgba_premultiplied(
                    (theme.text_muted.r() as f32 * (1.0 - phase) + theme.text.r() as f32 * phase)
                        as u8,
                    (theme.text_muted.g() as f32 * (1.0 - phase) + theme.text.g() as f32 * phase)
                        as u8,
                    (theme.text_muted.b() as f32 * (1.0 - phase) + theme.text.b() as f32 * phase)
                        as u8,
                    255,
                )
            } else {
                theme.text_muted
            };

            ui.add(
                egui::Label::new(
                    egui::RichText::new(header.label)
                        .size(theme.text_md)
                        .color(label_color),
                )
                .truncate(),
            );

            if chevron_resp.clicked() {
                state.expanded = !state.expanded;
            }
            chevron_resp
        });

        let row_resp = ui.interact(
            header_resp.response.rect,
            id.with("header_row"),
            egui::Sense::click(),
        );
        if row_resp.clicked() && !header_resp.inner.clicked() {
            state.expanded = !state.expanded;
        }

        if state.expanded {
            ui.add_space(theme.space_8);
            add_body(ui);
        }
    });

    state.expanded
}

// ============================================================================
// Thinking Block
// ============================================================================

/// Thinking block.
#[allow(dead_code)]
pub fn thinking_block(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: egui::Id,
    steps: &[String],
    is_reasoning: bool,
    state: &mut CollapsibleState,
) {
    let label = if is_reasoning {
        "Thinking...".to_string()
    } else {
        format!("Thought for {} steps", steps.len().saturating_sub(1))
    };
    if is_reasoning && !state.expanded {
        state.expanded = true;
    }
    let header = CollapsibleHeader::new(&label).loading(is_reasoning);
    collapsible(ui, theme, id, header, state, |ui| {
        for (i, step) in steps.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add_space(theme.space_4);
                ui.label(
                    egui::RichText::new(step)
                        .size(theme.text_md)
                        .color(theme.text_muted),
                );
            });
            if i < steps.len() - 1 {
                crate::design_system::gap(ui, crate::design_system::Space::S0);
            }
        }
    });
}

// ============================================================================
// Tool Group
// ============================================================================

/// Tool group.
#[allow(dead_code)]
pub fn tool_group(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: egui::Id,
    tools: &[ToolCallInfo],
    state: &mut CollapsibleState,
) {
    let header_label = if tools.len() == 1 {
        format!("Tool: {}", tools[0].name)
    } else {
        format!("{} tools", tools.len())
    };
    let header = CollapsibleHeader::new(&header_label).icon(tool_status_icon(tools));
    collapsible(ui, theme, id, header, state, |ui| {
        for (i, tool) in tools.iter().enumerate() {
            let is_last = i == tools.len() - 1;
            render_tool_row(ui, theme, tool, is_last);
        }
    });
}

#[allow(dead_code)]
fn render_tool_row(ui: &mut egui::Ui, theme: &Theme, tool: &ToolCallInfo, is_last: bool) {
    ui.horizontal(|ui| {
        let rail_width = 20.0;
        ui.allocate_ui_with_layout(
            egui::vec2(rail_width, ui.available_height()),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                let icon = match tool.inferred_status() {
                    ToolCallStatus::Running => crate::theme::ICON_HOURGLASS,
                    ToolCallStatus::Success => crate::theme::ICON_CHECK,
                    ToolCallStatus::Warning => crate::theme::ICON_WARNING,
                    ToolCallStatus::Error => crate::theme::ICON_X,
                };
                let color = match tool.inferred_status() {
                    ToolCallStatus::Running => theme.status_busy,
                    ToolCallStatus::Success => theme.ok,
                    ToolCallStatus::Warning => theme.warn,
                    ToolCallStatus::Error => theme.danger,
                };
                ui.label(
                    egui::RichText::new(icon)
                        .font(theme.font_icon(14.0))
                        .color(color),
                );
            },
        );
        ui.vertical(|ui| {
            ui.set_min_width((ui.available_width() - rail_width - 8.0).max(60.0));
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&tool.name)
                        .size(theme.text_sm)
                        .strong()
                        .color(theme.text_muted),
                );
                if let Some(ref result) = tool.result {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(crate::ui::render::truncate(result, 60))
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        )
                        .truncate(),
                    );
                }
            });
        });
    });
    if !is_last {
        crate::design_system::gap(ui, crate::design_system::Space::S1);
    }
}

#[allow(dead_code)]
fn tool_status_icon(tools: &[ToolCallInfo]) -> &str {
    if tools
        .iter()
        .any(|t| t.inferred_status() == ToolCallStatus::Running)
    {
        crate::theme::ICON_HOURGLASS
    } else if tools
        .iter()
        .any(|t| t.inferred_status() == ToolCallStatus::Error)
    {
        crate::theme::ICON_WARNING
    } else {
        crate::theme::ICON_CHECK
    }
}

// ============================================================================
// Subagent Group
// ============================================================================

/// Lifecycle status variants for subagent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SubagentStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

/// Holds subagent step state.
#[allow(dead_code)]
pub struct SubagentStep<'a> {
    pub ordinal: usize,
    pub description: &'a str,
    pub status: SubagentStatus,
}

/// Subagent group.
#[allow(dead_code)]
pub fn subagent_group(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: egui::Id,
    title: &str,
    steps: &[SubagentStep<'_>],
    state: &mut CollapsibleState,
) {
    let is_running = steps.iter().any(|s| s.status == SubagentStatus::Running);
    let header = CollapsibleHeader::new(title).loading(is_running);
    collapsible(ui, theme, id, header, state, |ui| {
        let _ = card(ui, theme, CardStyle::Nested, |ui| {
            for (i, step) in steps.iter().enumerate() {
                render_subagent_row(ui, theme, step);
                if i < steps.len() - 1 {
                    ui.add_space(2.0);
                }
            }
        });
    });
}

#[allow(dead_code)]
fn render_subagent_row(ui: &mut egui::Ui, theme: &Theme, step: &SubagentStep<'_>) {
    let (status_text, status_color) = match step.status {
        SubagentStatus::Pending => ("Pending", theme.text_dim),
        SubagentStatus::Running => ("Running", theme.text_dim),
        SubagentStatus::Succeeded => ("Done", theme.ok),
        SubagentStatus::Failed => ("Failed", theme.danger),
    };
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{:02}", step.ordinal))
                .size(theme.text_sm)
                .monospace()
                .color(theme.text_muted),
        );
        ui.add_space(theme.space_4);
        ui.add(
            egui::Label::new(
                egui::RichText::new(step.description)
                    .size(theme.text_sm)
                    .color(theme.text_muted),
            )
            .truncate(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(status_text)
                    .size(theme.text_sm)
                    .color(status_color),
            );
        });
    });
}

// ============================================================================
// Approval Dock
// ============================================================================

/// Holds approval request state.
pub struct ApprovalRequest {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub badge: Option<String>,
}

/// Renders the pending-approval dock.
pub fn approval_dock(
    ui: &mut egui::Ui,
    theme: &Theme,
    request: &ApprovalRequest,
) -> (Option<String>, Option<String>) {
    let mut denied = None;
    let mut allowed = None;

    let (_resp, _) = card(ui, theme, CardStyle::Approval, |ui| {
        ui.set_min_width(ui.available_width());

        ui.horizontal(|ui| {
            let dot_size = 6.0;
            let (dot_rect, _dot_resp) =
                ui.allocate_exact_size(egui::vec2(dot_size, dot_size), egui::Sense::hover());
            ui.painter()
                .circle_filled(dot_rect.center(), dot_size * 0.5, theme.warn);
            ui.add_space(theme.space_4);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&request.title)
                        .size(theme.text_md)
                        .strong()
                        .color(theme.text),
                )
                .truncate(),
            );
            if let Some(ref badge) = request.badge {
                ui.add_space(theme.space_4);
                let badge_frame = clarity_ui::design_system::Elevation::Elevated
                    .frame(theme)
                    .fill(theme.warn.linear_multiply(0.15))
                    // LAYOUT-EXEMPT: 21px pill badge radius; no pill token
                    // (radius_full is 999 and would change the shape).
                    .corner_radius(egui::CornerRadius::same(21));
                badge_frame.show(ui, |ui| {
                    clarity_ui::design_system::text_with_color(
                        ui,
                        badge,
                        clarity_ui::design_system::TextStyle::Small,
                        theme.warn,
                    );
                });
            }
        });

        crate::design_system::gap(ui, crate::design_system::Space::S1);

        egui::ScrollArea::vertical()
            .id_salt(ui.id().with("approval_detail"))
            .max_height(80.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(&request.detail)
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
            });

        crate::design_system::gap(ui, crate::design_system::Space::S1);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(theme.primary_button("Allow")).clicked() {
                allowed = Some(request.id.clone());
            }
            ui.add_space(theme.space_8);
            if ui.add(theme.secondary_button("Deny")).clicked() {
                denied = Some(request.id.clone());
            }
        });
    });

    (denied, allowed)
}

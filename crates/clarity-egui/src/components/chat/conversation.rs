//! Kimi-style conversation UI components — consolidated single file.
//!
//! Extracted from Kimi Desktop v3.0.15 `ConversationView` CSS/JS chunks.
//! Provides: message bubbles, thinking blocks, tool groups, subagent trackers,
//! approval dock, and knowledge panel.

use crate::theme::Theme;
use crate::ui::types::{ContentBlock, Message, Role, ToolCallInfo, ToolCallStatus};

// ============================================================================
// Primitives
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CardStyle {
    Knowledge,
    Approval,
    ListItem,
    Nested,
    Error,
}

impl CardStyle {
    fn frame(self, theme: &Theme) -> egui::Frame {
        match self {
            CardStyle::Knowledge => egui::Frame::new()
                .fill(theme.bg)
                .stroke(egui::Stroke::new(0.5, theme.border))
                .corner_radius(egui::CornerRadius::same(16)),
            CardStyle::Approval => egui::Frame::new()
                .fill(theme.bg_accent)
                .stroke(egui::Stroke::new(0.5, theme.border))
                .corner_radius(egui::CornerRadius::same(20))
                .shadow(theme.shadow_card),
            CardStyle::ListItem => egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(0.5, theme.border))
                .corner_radius(egui::CornerRadius::same(16)),
            CardStyle::Nested => egui::Frame::new()
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(12)),
            CardStyle::Error => egui::Frame::new()
                .fill(theme.error_bubble)
                .corner_radius(egui::CornerRadius::same(10)),
        }
    }

    fn inner_margin(self) -> egui::Margin {
        match self {
            CardStyle::Knowledge | CardStyle::Approval | CardStyle::ListItem => egui::Margin::same(16),
            CardStyle::Nested | CardStyle::Error => egui::Margin::symmetric(12, 10),
        }
    }
}

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
            let item_resp = ui.interact(ui.max_rect(), ui.id().with("card_hover"), egui::Sense::hover());
            if item_resp.hovered() {
                ui.painter().rect_filled(item_resp.rect, egui::CornerRadius::same(16), theme.bg_hover);
            }
            resp = Some(item_resp);
        }
        add_contents(ui)
    });

    let response = resp.unwrap_or_else(|| {
        ui.interact(inner.response.rect, ui.id().with("card"), egui::Sense::hover())
    });
    (response, inner.inner)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CollapsibleState {
    pub expanded: bool,
}

pub struct CollapsibleHeader<'a> {
    pub label: &'a str,
    pub is_loading: bool,
    pub icon: Option<&'a str>,
}

impl<'a> CollapsibleHeader<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, is_loading: false, icon: None }
    }
    #[allow(dead_code)]
    pub fn loading(mut self, v: bool) -> Self {
        self.is_loading = v;
        self
    }
    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }
}

pub fn collapsible<R>(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: egui::Id,
    header: CollapsibleHeader<'_>,
    state: &mut CollapsibleState,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> (bool, R) {
    let mut body_result = None;
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
                ui.label(egui::RichText::new(icon).font(theme.font_icon(16.0)).color(theme.text_muted));
            }

            let label_color = if header.is_loading {
                let t = ui.ctx().input(|i| i.time as f32);
                let phase = (t * 2.0).sin() * 0.5 + 0.5;
                egui::Color32::from_rgba_premultiplied(
                    (theme.text_muted.r() as f32 * (1.0 - phase) + theme.text.r() as f32 * phase) as u8,
                    (theme.text_muted.g() as f32 * (1.0 - phase) + theme.text.g() as f32 * phase) as u8,
                    (theme.text_muted.b() as f32 * (1.0 - phase) + theme.text.b() as f32 * phase) as u8,
                    255,
                )
            } else {
                theme.text_muted
            };

            ui.add(
                egui::Label::new(
                    egui::RichText::new(header.label).size(theme.text_md).color(label_color),
                )
                .truncate(),
            );

            if chevron_resp.clicked() {
                state.expanded = !state.expanded;
            }
            chevron_resp
        });

        let row_resp = ui.interact(header_resp.response.rect, id.with("header_row"), egui::Sense::click());
        if row_resp.clicked() && !header_resp.inner.clicked() {
            state.expanded = !state.expanded;
        }

        if state.expanded {
            ui.add_space(8.0);
            body_result = Some(add_body(ui));
        }
    });

    (state.expanded, body_result.expect("body rendered when expanded"))
}

pub fn streaming_cursor(ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
    let size = 14.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let t = ui.ctx().input(|i| i.time as f32);
    let alpha = ((t * 3.0).sin() * 0.5 + 0.5) * 255.0;
    let color = egui::Color32::from_rgba_premultiplied(theme.text.r(), theme.text.g(), theme.text.b(), alpha as u8);
    ui.painter().rect_filled(rect, egui::CornerRadius::same(2), color);
    resp
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
}

pub struct StepItem<'a> {
    pub title: &'a str,
    pub status: StepStatus,
    pub detail: Option<&'a str>,
}

pub fn step_list(
    ui: &mut egui::Ui,
    theme: &Theme,
    steps: &[StepItem<'_>],
    max_height: Option<f32>,
) -> f32 {
    let start_y = ui.cursor().min.y;
    let show = |ui: &mut egui::Ui| {
        for (i, step) in steps.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.set_min_height(20.0);
                let (icon, color) = match step.status {
                    StepStatus::Pending => (crate::theme::ICON_CIRCLE, theme.text_dim),
                    StepStatus::Running => {
                        let t = ui.ctx().input(|inp| inp.time as f32);
                        let phase = (t * 3.0).sin() * 0.5 + 0.5;
                        let pulse_color = egui::Color32::from_rgba_premultiplied(
                            (theme.text_muted.r() as f32 * (1.0 - phase) + theme.text.r() as f32 * phase) as u8,
                            (theme.text_muted.g() as f32 * (1.0 - phase) + theme.text.g() as f32 * phase) as u8,
                            (theme.text_muted.b() as f32 * (1.0 - phase) + theme.text.b() as f32 * phase) as u8,
                            255,
                        );
                        (crate::theme::ICON_HOURGLASS, pulse_color)
                    }
                    StepStatus::Done => (crate::theme::ICON_CHECK, theme.ok),
                    StepStatus::Failed => (crate::theme::ICON_WARNING, theme.danger),
                };
                ui.label(egui::RichText::new(icon).font(theme.font_icon(16.0)).color(color));
                ui.add_space(4.0);
                let mut title = egui::RichText::new(step.title)
                    .size(theme.text_sm)
                    .color(if step.status == StepStatus::Pending { theme.text_dim } else { theme.text_muted });
                if step.status == StepStatus::Done {
                    title = title.strikethrough();
                }
                ui.add(egui::Label::new(title).truncate());
            });
            if let Some(detail) = step.detail {
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.label(egui::RichText::new(detail).size(theme.text_xs).color(theme.text_dim));
                });
            }
            if i < steps.len() - 1 {
                ui.add_space(4.0);
            }
        }
    };

    if let Some(max_h) = max_height {
        egui::ScrollArea::vertical()
            .id_salt(ui.id().with("step_list_scroll"))
            .max_height(max_h)
            .show(ui, show);
    } else {
        show(ui);
    }
    ui.cursor().min.y - start_y
}

// ============================================================================
// Message Bubble
// ============================================================================

pub fn message_bubble(
    ui: &mut egui::Ui,
    theme: &Theme,
    msg: &Message,
    msg_idx: usize,
    is_streaming: bool,
    on_copy: &mut Option<String>,
    on_edit: &mut Option<usize>,
    on_regenerate: &mut Option<usize>,
) -> f32 {
    let start_y = ui.cursor().min.y;
    match msg.role {
        Role::User => render_user_row(ui, theme, msg, msg_idx, on_copy, on_edit),
        Role::Agent => render_assistant_row(ui, theme, msg, msg_idx, is_streaming, on_copy, on_regenerate),
    }
    ui.add_space(theme.space_12);
    ui.cursor().min.y - start_y
}

fn render_user_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    msg: &Message,
    msg_idx: usize,
    on_copy: &mut Option<String>,
    on_edit: &mut Option<usize>,
) {
    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        let max_width = (ui.available_width() * 0.80).max(280.0);
        ui.set_max_width(max_width);

        let bubble_frame = egui::Frame::new()
            .fill(theme.user_bubble)
            .corner_radius(egui::CornerRadius::same(16))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(18, 14));

        let bubble_resp = bubble_frame.show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.set_min_width(48.0);
                if msg.parsed.is_empty() {
                    ui.label(egui::RichText::new(&msg.content).size(theme.text_md).color(theme.text));
                } else {
                    crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.text);
                }
            });
        });

        let hovered = ui.ctx().input(|i| {
            i.pointer.hover_pos().map_or(false, |p| bubble_resp.response.rect.contains(p))
        });

        if hovered {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let copy_btn = icon_button(ui, theme, crate::theme::ICON_COPY, "Copy");
                if copy_btn.clicked() { *on_copy = Some(msg.content.clone()); }
                ui.add_space(4.0);
                let edit_btn = icon_button(ui, theme, crate::theme::ICON_EDIT, "Edit");
                if edit_btn.clicked() { *on_edit = Some(msg_idx); }
            });
        }
    });
}

fn render_assistant_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    msg: &Message,
    msg_idx: usize,
    is_streaming: bool,
    on_copy: &mut Option<String>,
    on_regenerate: &mut Option<usize>,
) {
    ui.horizontal(|ui| {
        let avatar_size = 32.0;
        let (avatar_rect, _avatar_resp) = ui.allocate_exact_size(egui::vec2(avatar_size, avatar_size), egui::Sense::hover());
        let center = avatar_rect.center();
        ui.painter().circle_filled(center, avatar_size * 0.5, theme.accent);
        let label = ui.fonts(|f| {
            f.layout("K".to_string(), theme.font_bold(theme.text_sm), egui::Color32::WHITE, f32::INFINITY)
        });
        let label_pos = center - label.rect.size() * 0.5;
        ui.painter().galley(label_pos, label, egui::Color32::WHITE);

        ui.add_space(8.0);

        ui.vertical(|ui| {
            ui.add_space(4.0);
            let max_width = ui.available_width();
            ui.set_max_width(max_width);

            if msg.parsed.is_empty() && !msg.content.is_empty() {
                ui.label(egui::RichText::new(&msg.content).size(theme.text_md).color(theme.text));
                if is_streaming {
                    ui.horizontal(|ui| { streaming_cursor(ui, theme); });
                }
            } else if !msg.blocks.is_empty() {
                render_blocks(ui, theme, msg, is_streaming);
            } else {
                crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.text);
                if is_streaming {
                    ui.horizontal(|ui| { streaming_cursor(ui, theme); });
                }
            }

            let hovered = ui.ctx().input(|i| {
                i.pointer.hover_pos().map_or(false, |p| ui.max_rect().contains(p))
            });

            if hovered {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    let copy_btn = icon_button(ui, theme, crate::theme::ICON_COPY, "Copy");
                    if copy_btn.clicked() { *on_copy = Some(msg.content.clone()); }
                    ui.add_space(4.0);
                    let regen_btn = icon_button(ui, theme, crate::theme::ICON_REFRESH, "Regenerate");
                    if regen_btn.clicked() { *on_regenerate = Some(msg_idx); }
                });
            }
        });
    });
}

fn render_blocks(ui: &mut egui::Ui, theme: &Theme, msg: &Message, is_streaming: bool) {
    let visible_blocks: Vec<_> = msg.blocks.iter().filter(|b| matches!(b,
        ContentBlock::Text { .. } | ContentBlock::Code { .. } | ContentBlock::Think { .. } |
        ContentBlock::Plan { .. } | ContentBlock::ToolResult { .. } | ContentBlock::FilePreview { .. }
    )).collect();

    for block in visible_blocks {
        match block {
            ContentBlock::Text { text } => {
                let parsed = crate::ui::markdown::parse_markdown(text);
                crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.text);
            }
            ContentBlock::Code { language, code } => {
                render_code_block(ui, theme, language, code);
            }
            ContentBlock::Think { steps } => {
                let id = ui.id().with("think");
                let mut state = ui.ctx().data_mut(|d| *d.get_temp_mut_or(id, CollapsibleState::default()));
                let header_text = format!("Thinking ({})", steps.len());
                let header = CollapsibleHeader::new(&header_text);
                collapsible(ui, theme, id, header, &mut state, |ui| {
                    for step in steps {
                        ui.label(egui::RichText::new(step).size(theme.text_md).color(theme.text_muted));
                    }
                });
                ui.ctx().data_mut(|d| d.insert_temp(id, state));
            }
            ContentBlock::Plan { title, steps } => {
                ui.label(egui::RichText::new(title).size(theme.text_base).strong().color(theme.text));
                for step in steps {
                    ui.label(egui::RichText::new(format!("• {}", step)).size(theme.text_sm).color(theme.text_muted));
                }
            }
            ContentBlock::ToolResult { name, output, .. } => {
                let id = ui.id().with(("tool", name));
                let mut state = ui.ctx().data_mut(|d| *d.get_temp_mut_or(id, CollapsibleState::default()));
                let header_text = format!("🔧 {}", name);
                let header = CollapsibleHeader::new(&header_text).icon(crate::theme::ICON_WRENCH);
                collapsible(ui, theme, id, header, &mut state, |ui| {
                    let parsed = crate::ui::markdown::parse_markdown(output);
                    crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.text_muted);
                });
                ui.ctx().data_mut(|d| d.insert_temp(id, state));
            }
            ContentBlock::FilePreview { path, content } => {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::theme::ICON_FILE).font(theme.font_icon(theme.text_sm)).color(theme.text_muted));
                    ui.label(egui::RichText::new(path).size(theme.text_sm).strong().color(theme.text_muted));
                });
                let parsed = crate::ui::markdown::parse_markdown(content);
                crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.text_muted);
            }
            _ => {}
        }
        ui.add_space(theme.space_8);
    }

    if is_streaming && msg.blocks.is_empty() {
        ui.horizontal(|ui| { streaming_cursor(ui, theme); });
    }
}

fn render_code_block(ui: &mut egui::Ui, theme: &Theme, language: &str, code: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(language).size(theme.text_xs).color(theme.text_dim).monospace());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let copy_btn = icon_button(ui, theme, crate::theme::ICON_COPY, "Copy code");
            if copy_btn.clicked() { ui.ctx().copy_text(code.to_string()); }
        });
    });
    egui::Frame::new()
        .fill(theme.bg_hover)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(egui::RichText::new(code).font(theme.font_mono(theme.text_sm)).color(theme.text));
            });
        });
}

fn icon_button(ui: &mut egui::Ui, theme: &Theme, icon: &str, tooltip: &str) -> egui::Response {
    let btn = egui::Button::new(
        egui::RichText::new(icon).font(theme.font_icon(theme.text_sm)).color(theme.text_muted),
    )
    .fill(egui::Color32::TRANSPARENT)
    .stroke(egui::Stroke::NONE)
    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
    ui.add(btn).on_hover_text(tooltip)
}

// ============================================================================
// Thinking Block
// ============================================================================

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
                ui.add_space(4.0);
                ui.label(egui::RichText::new(step).size(theme.text_md).color(theme.text_muted));
            });
            if i < steps.len() - 1 {
                ui.add_space(theme.space_4);
            }
        }
    });
}

// ============================================================================
// Tool Group
// ============================================================================

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
                ui.label(egui::RichText::new(icon).font(theme.font_icon(14.0)).color(color));
            },
        );
        ui.vertical(|ui| {
            ui.set_min_width((ui.available_width() - rail_width - 8.0).max(60.0));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&tool.name).size(theme.text_sm).strong().color(theme.text_muted));
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
        ui.add_space(theme.space_8);
    }
}

#[allow(dead_code)]
fn tool_status_icon(tools: &[ToolCallInfo]) -> &str {
    if tools.iter().any(|t| t.inferred_status() == ToolCallStatus::Running) {
        crate::theme::ICON_HOURGLASS
    } else if tools.iter().any(|t| t.inferred_status() == ToolCallStatus::Error) {
        crate::theme::ICON_WARNING
    } else {
        crate::theme::ICON_CHECK
    }
}

// ============================================================================
// Subagent Group
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SubagentStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

#[allow(dead_code)]
pub struct SubagentStep<'a> {
    pub ordinal: usize,
    pub description: &'a str,
    pub status: SubagentStatus,
}

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
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(step.description).size(theme.text_sm).color(theme.text_muted),
            )
            .truncate(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(status_text).size(theme.text_sm).color(status_color));
        });
    });
}

// ============================================================================
// Approval Dock
// ============================================================================

pub struct ApprovalRequest {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub badge: Option<String>,
}

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
            let (dot_rect, _dot_resp) = ui.allocate_exact_size(egui::vec2(dot_size, dot_size), egui::Sense::hover());
            ui.painter().circle_filled(dot_rect.center(), dot_size * 0.5, theme.warn);
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&request.title).size(theme.text_md).strong().color(theme.text),
                )
                .truncate(),
            );
            if let Some(ref badge) = request.badge {
                ui.add_space(4.0);
                let badge_frame = egui::Frame::new()
                    .fill(theme.warn.linear_multiply(0.15))
                    .corner_radius(egui::CornerRadius::same(21));
                badge_frame.show(ui, |ui| {
                    ui.label(egui::RichText::new(badge).size(theme.text_xs).color(theme.warn));
                });
            }
        });

        ui.add_space(theme.space_8);

        egui::ScrollArea::vertical()
            .id_salt(ui.id().with("approval_detail"))
            .max_height(80.0)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(&request.detail).size(theme.text_sm).color(theme.text_muted));
            });

        ui.add_space(theme.space_8);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let allow_btn = egui::Button::new(
                egui::RichText::new("Allow").size(theme.text_sm).strong().color(theme.bg),
            )
            .fill(theme.text)
            .corner_radius(egui::CornerRadius::same(10));
            if ui.add(allow_btn).clicked() {
                allowed = Some(request.id.clone());
            }
            ui.add_space(8.0);
            let deny_btn = egui::Button::new(
                egui::RichText::new("Deny").size(theme.text_sm).strong().color(theme.text),
            )
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(10));
            if ui.add(deny_btn).clicked() {
                denied = Some(request.id.clone());
            }
        });
    });

    (denied, allowed)
}

// ============================================================================
// Knowledge Panel
// ============================================================================

pub struct ContextFile {
    pub name: String,
    pub icon: Option<String>,
}

pub fn knowledge_panel(
    ui: &mut egui::Ui,
    theme: &Theme,
    plan_title: &str,
    plan_steps: &[StepItem<'_>],
    context_files: &[ContextFile],
) {
    ui.set_width(386.0);
    ui.set_min_height(ui.available_height());

    ui.vertical(|ui| {
        ui.set_width(354.0);
        ui.set_min_width(354.0);

        if !plan_steps.is_empty() {
            let _ = card(ui, theme, CardStyle::Knowledge, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(plan_title).size(theme.text_sm).strong().color(theme.text),
                    );
                });
                ui.add_space(theme.space_8);
                step_list(ui, theme, plan_steps, Some(340.0));
            });
        }

        ui.add_space(theme.space_16);

        if !context_files.is_empty() {
            let _ = card(ui, theme, CardStyle::Knowledge, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Context").size(theme.text_sm).strong().color(theme.text),
                    );
                });
                ui.add_space(theme.space_8);
                for (i, file) in context_files.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let icon = file.icon.as_deref().unwrap_or(crate::theme::ICON_FILE);
                        ui.label(
                            egui::RichText::new(icon).font(theme.font_icon(theme.text_sm)).color(theme.text_muted),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&file.name).size(theme.text_sm).color(theme.text_muted),
                            )
                            .truncate(),
                        );
                    });
                    if i < context_files.len() - 1 {
                        ui.add_space(4.0);
                    }
                }
            });
        }
    });
}

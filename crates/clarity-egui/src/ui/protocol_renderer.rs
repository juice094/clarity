use crate::theme::Theme;
use clarity_wire::{ButtonStyle, TextRole, UserAction, ViewCommand};

/// Render a slice of `ViewCommand`s into egui widgets, collecting any user actions.
pub fn render_view_commands(
    ui: &mut egui::Ui,
    commands: &[ViewCommand],
    theme: &Theme,
    actions: &mut Vec<UserAction>,
) {
    for cmd in commands {
        render_single(ui, cmd, theme, actions);
    }
}

fn render_single(
    ui: &mut egui::Ui,
    cmd: &ViewCommand,
    theme: &Theme,
    actions: &mut Vec<UserAction>,
) {
    match cmd {
        ViewCommand::VStack { children } => {
            ui.vertical(|ui| {
                render_view_commands(ui, children, theme, actions);
            });
        }
        ViewCommand::HStack { children } => {
            ui.horizontal(|ui| {
                render_view_commands(ui, children, theme, actions);
            });
        }
        ViewCommand::Text { content, role, size } => {
            let mut text = egui::RichText::new(content).size(*size).color(theme.text);
            if matches!(role, TextRole::Title) {
                text = text.strong();
            }
            ui.label(text);
        }
        ViewCommand::TextInput {
            id,
            value,
            placeholder,
            password,
            width,
        } => {
            let mut local = value.clone();
            let response = ui.add_sized(
                egui::vec2(*width, 28.0),
                egui::TextEdit::singleline(&mut local)
                    .password(*password)
                    .hint_text(placeholder.as_str())
                    .text_color(theme.text),
            );
            if response.changed() && local != *value {
                actions.push(UserAction::TextInputChange {
                    id: id.clone(),
                    value: local,
                });
            }
        }
        ViewCommand::ComboBox {
            id,
            selected_value,
            options,
            width,
        } => {
            let selected_label = options
                .iter()
                .find(|(v, _)| v == selected_value)
                .map(|(_, l)| l.as_str())
                .unwrap_or(selected_value.as_str());
            let mut current = selected_value.clone();
            egui::ComboBox::from_id_salt(id.as_str())
                .selected_text(selected_label)
                .width(*width)
                .show_ui(ui, |ui| {
                    for (value, label) in options {
                        ui.selectable_value(&mut current, value.clone(), label.as_str());
                    }
                });
            if current != *selected_value {
                actions.push(UserAction::ComboChange {
                    id: id.clone(),
                    selected: current,
                });
            }
        }
        ViewCommand::Button {
            id,
            label,
            style,
            min_width,
            min_height,
        } => {
            let fill = match style {
                ButtonStyle::Primary => theme.accent,
                ButtonStyle::Secondary => theme.border,
                ButtonStyle::Danger => theme.danger,
            };
            let button = egui::Button::new(egui::RichText::new(label).color(theme.text))
                .fill(fill)
                .min_size(egui::vec2(*min_width, *min_height))
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
            if ui.add(button).clicked() {
                actions.push(UserAction::ButtonClick { id: id.clone() });
            }
        }
        ViewCommand::Space { height } => {
            ui.add_space(*height);
        }
    }
}

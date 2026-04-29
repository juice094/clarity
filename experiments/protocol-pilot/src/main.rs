mod protocol;

use protocol::{TextRole, UserAction, ViewCommand};
use std::time::{Duration, Instant};

// ============================================================================
// ViewModel: holds state and produces ViewCommands
// ============================================================================

#[derive(Clone, Debug)]
pub struct SettingsViewModel {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub local_model_path: String,
    pub approval_mode: String,
    pub providers: Vec<(String, String, Vec<String>)>,
}

impl Default for SettingsViewModel {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: "gpt-4".into(),
            api_key: String::new(),
            local_model_path: String::new(),
            approval_mode: "interactive".into(),
            providers: vec![
                ("openai".into(), "OpenAI".into(), vec!["gpt-4".into(), "gpt-3.5".into()]),
                ("ollama".into(), "Ollama".into(), vec!["llama2".into(), "mistral".into()]),
                ("deepseek".into(), "DeepSeek".into(), vec!["deepseek-chat".into()]),
            ],
        }
    }
}

impl SettingsViewModel {
    /// Produce a declarative command tree for the current state.
    pub fn render(&self) -> Vec<ViewCommand> {
        let current_models = self
            .providers
            .iter()
            .find(|(k, _, _)| k == &self.provider)
            .map(|(_, _, m)| m.clone())
            .unwrap_or_default();

        vec![
            ViewCommand::VStack {
                children: vec![
                    row("Provider", ViewCommand::ComboBox {
                        id: "provider".into(),
                        selected: self.provider.clone(),
                        options: self.providers.iter().map(|(_, label, _)| label.clone()).collect(),
                        width: 200.0,
                    }),
                    ViewCommand::Space { height: 8.0 },
                    row("Model", ViewCommand::ComboBox {
                        id: "model".into(),
                        selected: self.model.clone(),
                        options: current_models,
                        width: 200.0,
                    }),
                    ViewCommand::Space { height: 8.0 },
                    row("API Key", ViewCommand::TextInput {
                        id: "api_key".into(),
                        value: self.api_key.clone(),
                        placeholder: "sk-...".into(),
                        password: true,
                        width: 200.0,
                    }),
                    ViewCommand::Space { height: 8.0 },
                    row("Local Model Path", ViewCommand::TextInput {
                        id: "local_model_path".into(),
                        value: self.local_model_path.clone(),
                        placeholder: "/path/to/model.gguf".into(),
                        password: false,
                        width: 200.0,
                    }),
                    ViewCommand::Space { height: 8.0 },
                    row("Approval Mode", ViewCommand::ComboBox {
                        id: "approval_mode".into(),
                        selected: self.approval_mode.clone(),
                        options: vec![
                            "Interactive — Approve each tool call".into(),
                            "Yolo — Auto-approve all".into(),
                            "Plan — Review plan before execution".into(),
                        ],
                        width: 200.0,
                    }),
                    ViewCommand::Space { height: 16.0 },
                    ViewCommand::HStack {
                        children: vec![
                            ViewCommand::Button {
                                id: "cancel".into(),
                                label: "Cancel".into(),
                                min_width: 80.0,
                                min_height: 32.0,
                            },
                            ViewCommand::Button {
                                id: "save".into(),
                                label: "Save".into(),
                                min_width: 80.0,
                                min_height: 32.0,
                            },
                        ],
                    },
                ],
            },
        ]
    }

    /// Apply a user action to mutate state.
    pub fn apply_action(&mut self, action: &UserAction) {
        match action {
            UserAction::ComboChange { id, selected } => match id.as_str() {
                "provider" => {
                    self.provider = selected.clone();
                    // Auto-select first model
                    if let Some((_, _, models)) = self.providers.iter().find(|(k, _, _)| k == selected) {
                        if let Some(first) = models.first() {
                            self.model = first.clone();
                        }
                    }
                }
                "model" => self.model = selected.clone(),
                "approval_mode" => self.approval_mode = selected.clone(),
                _ => {}
            },
            UserAction::TextInputChange { id, value } => match id.as_str() {
                "api_key" => self.api_key = value.clone(),
                "local_model_path" => self.local_model_path = value.clone(),
                _ => {}
            },
            UserAction::ButtonClick { id } => {
                if id == "save" {
                    // Simulate save logic
                }
            }
        }
    }
}

fn row(label: &str, control: ViewCommand) -> ViewCommand {
    ViewCommand::HStack {
        children: vec![
            ViewCommand::Text {
                content: label.into(),
                role: TextRole::Label,
                size: 13.0,
            },
            control,
        ],
    }
}

// ============================================================================
// Protocol Renderer: translates ViewCommands into egui draw calls
// ============================================================================

pub fn render_commands(
    ui: &mut egui::Ui,
    commands: &[ViewCommand],
    actions: &mut Vec<UserAction>,
) {
    for cmd in commands {
        match cmd {
            ViewCommand::VStack { children } => {
                ui.vertical(|ui| render_commands(ui, children, actions));
            }
            ViewCommand::HStack { children } => {
                ui.horizontal(|ui| render_commands(ui, children, actions));
            }
            ViewCommand::Text { content, role: _, size } => {
                ui.label(egui::RichText::new(content).size(*size));
            }
            ViewCommand::TextInput { id, value, placeholder, password, width } => {
                let mut temp = value.clone();
                let response = ui.add_sized(
                    egui::vec2(*width, 28.0),
                    egui::TextEdit::singleline(&mut temp)
                        .hint_text(placeholder)
                        .password(*password),
                );
                if response.changed() && temp != *value {
                    actions.push(UserAction::TextInputChange {
                        id: id.clone(),
                        value: temp,
                    });
                }
            }
            ViewCommand::ComboBox { id, selected, options, width } => {
                let mut current = selected.clone();
                egui::ComboBox::from_id_salt(id.as_str())
                    .selected_text(&current)
                    .width(*width)
                    .show_ui(ui, |ui| {
                        for opt in options {
                            if ui.selectable_value(&mut current, opt.clone(), opt).changed() {
                                actions.push(UserAction::ComboChange {
                                    id: id.clone(),
                                    selected: opt.clone(),
                                });
                            }
                        }
                    });
            }
            ViewCommand::Button { id, label, min_width, min_height } => {
                if ui
                    .add_sized(
                        egui::vec2(*min_width, *min_height),
                        egui::Button::new(label),
                    )
                    .clicked()
                {
                    actions.push(UserAction::ButtonClick { id: id.clone() });
                }
            }
            ViewCommand::Space { height } => {
                ui.add_space(*height);
            }
        }
    }
}

// ============================================================================
// Direct Renderer: baseline — direct egui calls (current approach)
// ============================================================================

pub fn render_direct(ui: &mut egui::Ui, vm: &SettingsViewModel, actions: &mut Vec<UserAction>) {
    ui.vertical(|ui| {
        // Provider
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Provider").size(13.0));
            let mut current = vm.provider.clone();
            egui::ComboBox::from_id_salt("provider_direct")
                .selected_text(&current)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (_, label, _) in &vm.providers {
                        if ui.selectable_value(&mut current, label.clone(), label).changed() {
                            actions.push(UserAction::ComboChange {
                                id: "provider".into(),
                                selected: label.clone(),
                            });
                        }
                    }
                });
        });
        ui.add_space(8.0);

        // Model
        let current_models = vm
            .providers
            .iter()
            .find(|(k, _, _)| k == &vm.provider)
            .map(|(_, _, m)| m.clone())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Model").size(13.0));
            let mut current = vm.model.clone();
            egui::ComboBox::from_id_salt("model_direct")
                .selected_text(&current)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for m in &current_models {
                        if ui.selectable_value(&mut current, m.clone(), m).changed() {
                            actions.push(UserAction::ComboChange {
                                id: "model".into(),
                                selected: m.clone(),
                            });
                        }
                    }
                });
        });
        ui.add_space(8.0);

        // API Key
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("API Key").size(13.0));
            let mut key = vm.api_key.clone();
            let r = ui.add_sized(
                egui::vec2(200.0, 28.0),
                egui::TextEdit::singleline(&mut key).password(true),
            );
            if r.changed() {
                actions.push(UserAction::TextInputChange {
                    id: "api_key".into(),
                    value: key,
                });
            }
        });
        ui.add_space(8.0);

        // Local Model Path
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Local Model Path").size(13.0));
            let mut path = vm.local_model_path.clone();
            let r = ui.add_sized(
                egui::vec2(200.0, 28.0),
                egui::TextEdit::singleline(&mut path),
            );
            if r.changed() {
                actions.push(UserAction::TextInputChange {
                    id: "local_model_path".into(),
                    value: path,
                });
            }
        });
        ui.add_space(8.0);

        // Approval Mode
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Approval Mode").size(13.0));
            let mut current = vm.approval_mode.clone();
            egui::ComboBox::from_id_salt("approval_direct")
                .selected_text(&current)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for opt in ["interactive", "yolo", "plan"] {
                        if ui.selectable_value(&mut current, opt.into(), opt).changed() {
                            actions.push(UserAction::ComboChange {
                                id: "approval_mode".into(),
                                selected: opt.into(),
                            });
                        }
                    }
                });
        });
        ui.add_space(16.0);

        // Buttons
        ui.horizontal(|ui| {
            if ui.add_sized(egui::vec2(80.0, 32.0), egui::Button::new("Cancel")).clicked() {
                actions.push(UserAction::ButtonClick { id: "cancel".into() });
            }
            if ui.add_sized(egui::vec2(80.0, 32.0), egui::Button::new("Save")).clicked() {
                actions.push(UserAction::ButtonClick { id: "save".into() });
            }
        });
    });
}

// ============================================================================
// Benchmark harness
// ============================================================================

const FRAMES: usize = 1000;

fn bench<F>(name: &str, mut f: F)
where
    F: FnMut(&mut egui::Ui),
{
    let ctx = egui::Context::default();
    let mut times = Vec::with_capacity(FRAMES);

    for _ in 0..FRAMES {
        let raw_input = egui::RawInput::default();
        let start = Instant::now();
        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                f(ui);
            });
        });
        times.push(start.elapsed());
    }

    let total: Duration = times.iter().sum();
    let avg = total / FRAMES as u32;
    let min = times.iter().min().unwrap();
    let max = times.iter().max().unwrap();
    let mut sorted = times.clone();
    sorted.sort();
    let p99 = sorted[(FRAMES as f64 * 0.99) as usize];

    println!("\n=== {} ===", name);
    println!("  Frames:     {}", FRAMES);
    println!("  Total:      {:?}", total);
    println!("  Avg/frame:  {:?}", avg);
    println!("  Min:        {:?}", min);
    println!("  Max:        {:?}", max);
    println!("  P99:        {:?}", p99);
}

fn main() {
    println!("Protocol Pilot — Performance Benchmark");
    println!("Comparing direct egui rendering vs protocol-driven rendering\n");

    // Warm-up: let egui initialize fonts/cache
    let ctx = egui::Context::default();
    for _ in 0..10 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("warmup");
            });
        });
    }

    // Baseline: Direct rendering
    let vm = SettingsViewModel::default();
    bench("Direct (baseline)", |ui| {
        let mut actions = Vec::new();
        render_direct(ui, &vm, &mut actions);
    });

    // Protocol: ViewModel → ViewCommand → Protocol Renderer
    let vm = SettingsViewModel::default();
    bench("Protocol-driven", |ui| {
        let mut actions = Vec::new();
        let commands = vm.render();
        render_commands(ui, &commands, &mut actions);
    });

    // Micro-benchmark: ViewCommand generation only (no egui)
    let vm = SettingsViewModel::default();
    let start = Instant::now();
    for _ in 0..FRAMES {
        let _ = vm.render();
    }
    let gen_total = start.elapsed();
    println!("\n=== ViewCommand generation only (no egui) ===");
    println!("  Total:      {:?}", gen_total);
    println!("  Avg/frame:  {:?}", gen_total / FRAMES as u32);

    println!("\nDone.");
}

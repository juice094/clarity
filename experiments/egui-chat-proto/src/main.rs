use eframe::egui;
use std::time::{Duration, Instant};

// ============================================================================
// Clarity Chat Proto — egui MVP
// ============================================================================
// Goal: Validate whether egui can support a conversational chat interface
// within a reasonable time budget (2 weeks target).
//
// Scope:
//   ✅ Message history with user/AI bubbles
//   ✅ Multi-line input + send button
//   ✅ Simulated AI response (echo with delay)
//   ✅ Auto-scroll to bottom
//   ❌ Real LLM integration (out of scope for UI proto)
//   ✅ Streaming token-by-token (Phase 2)
//   ❌ File attachments / images
// ============================================================================

struct Message {
    text: String,
    is_user: bool,
    #[allow(dead_code)]
    timestamp: Instant,
}

struct App {
    messages: Vec<Message>,
    input: String,
    pending_ai: bool,
    ai_timer: Option<Instant>,
    ai_text: String,
    log: Vec<String>,
    start: Instant,
    frame_count: u64,
    last_fps_time: f64,
    fps: f64,
}

impl App {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            pending_ai: false,
            ai_timer: None,
            ai_text: String::new(),
            log: Vec::new(),
            start: Instant::now(),
            frame_count: 0,
            last_fps_time: 0.0,
            fps: 0.0,
        }
    }

    fn log(&mut self, event: &str) {
        let t = self.start.elapsed().as_secs_f64();
        self.log.push(format!("[{:.3}s] {}", t, event));
    }

    fn send(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.messages.push(Message {
            text: text.clone(),
            is_user: true,
            timestamp: Instant::now(),
        });
        self.log(&format!("USER msg={} chars={}", self.messages.len(), text.len()));
        self.input.clear();

        // Trigger simulated AI response
        self.pending_ai = true;
        self.ai_timer = Some(Instant::now());
        self.ai_text = format!("Echo: {}", text);
    }

    fn tick_ai(&mut self) {
        if !self.pending_ai {
            return;
        }
        let now = Instant::now();
        if let Some(timer) = self.ai_timer {
            if timer.elapsed() >= Duration::from_millis(50) {
                // Streaming: append one char at a time
                let chars: Vec<char> = self.ai_text.chars().collect();
                if let Some(last) = self.messages.last_mut() {
                    if !last.is_user {
                        let current_len = last.text.len();
                        if current_len < self.ai_text.len() {
                            let next = chars[current_len..].iter().take(3).collect::<String>();
                            last.text.push_str(&next);
                            self.ai_timer = Some(now);
                            return;
                        }
                    }
                }
                // Start new AI message or finished
                if self.messages.last().map_or(true, |m| m.is_user) {
                    self.messages.push(Message {
                        text: chars.get(0..1).unwrap_or_default().iter().collect(),
                        is_user: false,
                        timestamp: now,
                    });
                    self.log(&format!("AI  msg={} streaming started", self.messages.len()));
                } else if self.messages.last().unwrap().text.len() >= self.ai_text.len() {
                    self.log(&format!("AI  msg={} done chars={}", self.messages.len(), self.ai_text.len()));
                    self.pending_ai = false;
                    self.ai_timer = None;
                    return;
                }
                self.ai_timer = Some(now);
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.frame_count += 1;
        if now - self.last_fps_time >= 1.0 {
            self.fps = self.frame_count as f64 / (now - self.last_fps_time);
            self.frame_count = 0;
            self.last_fps_time = now;
        }

        self.tick_ai();
        if self.pending_ai {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Clarity Chat Proto — egui MVP");
            ui.label(format!(
                "FPS: {:.1} | Messages: {} | Status: {}",
                self.fps,
                self.messages.len(),
                if self.pending_ai { "AI typing..." } else { "Ready" }
            ));

            // ---- Message history ----
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(460.0)
                .show(ui, |ui| {
                    for msg in &self.messages {
                        chat_bubble(ui, msg);
                    }
                    if self.pending_ai {
                        typing_indicator(ui);
                    }
                });

            ui.separator();

            // ---- Input area ----
            ui.horizontal(|ui| {
                let available_width = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(available_width - 80.0, 60.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.label("Input:");
                        ui.text_edit_multiline(&mut self.input);
                    },
                );
                ui.vertical(|ui| {
                    ui.add_space(18.0);
                    let btn = ui.button("Send");
                    if btn.clicked() {
                        self.send();
                    }
                });
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.log.push(format!(
            "[EXIT] messages={} duration={:.1}s",
            self.messages.len(),
            self.start.elapsed().as_secs_f64()
        ));
        let text = self.log.join("\n");
        if let Err(e) = std::fs::write("chat_proto_log.txt", text) {
            eprintln!("Log write failed: {}", e);
        } else {
            println!("Log saved to chat_proto_log.txt");
        }
    }
}

// ---- Bubble rendering ----

fn chat_bubble(ui: &mut egui::Ui, msg: &Message) {
    let (align, bg, name, text_color) = if msg.is_user {
        (
            egui::Align::RIGHT,
            egui::Color32::from_rgb(0, 120, 255),
            "You",
            egui::Color32::WHITE,
        )
    } else {
        (
            egui::Align::LEFT,
            egui::Color32::from_rgb(235, 235, 235),
            "AI",
            egui::Color32::BLACK,
        )
    };

    ui.with_layout(egui::Layout::top_down(align), |ui| {
        egui::Frame::group(ui.style())
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(10))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(name).size(10.0).color(text_color).weak());
                ui.label(egui::RichText::new(&msg.text).color(text_color));
            });
    });
    ui.add_space(6.0);
}

fn typing_indicator(ui: &mut egui::Ui) {
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgb(235, 235, 235))
            .corner_radius(egui::CornerRadius::same(10))
            .show(ui, |ui| {
                ui.label("...");
            });
    });
    ui.add_space(6.0);
}

// ---- Entry ----

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([600.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Clarity Chat Proto",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

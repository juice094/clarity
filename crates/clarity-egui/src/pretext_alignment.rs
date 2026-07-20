//! Pretext alignment regression tests + release performance benchmark.
//!
//! These tests verify that `estimate_height()` predicts the same height that
//! `message_bubble()` actually consumes, and provide an ignored release-mode
//! benchmark for 1000 virtual messages.

use crate::pretext::EguiFontMetrics;
use crate::theme::Theme;
use crate::ui::render::{estimate_height, message_bubble};
use crate::ui::types::{Message, Role};
use std::time::Instant;

/// Diverse sample set covering Latin, CJK, code chips, mentions, and mixed runs.
const ALIGNMENT_SAMPLES: &[&str] = &[
    "Hello world",
    "你好世界",
    "Mixed 中英混排 text with 中文插入。",
    "🦊 emoji 宽度 👨‍👩‍👧‍👦 test",
    "A very long English sentence that should wrap into at least two or three lines when the max width is constrained to a modest value.",
    "这是一段比较长的中文文本，用来验证pretext在CJK字符上的换行和高度预测是否准确。",
    "Use `inline_code` for variables and call @agent for help.",
    "Check `this_long_code_identifier` and mention @very_long_user_name in the same message.",
    "@system please review `src/main.rs` and `src/lib.rs` before proceeding.",
    "Multiple `code1`, `code2`, and `code3` chips on one line.",
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
    "路径 `C:\\Users\\demo\\file.txt` 在Windows下需要转义。",
    "Rust 类型系统 `Result<T, E>` 与 `Option<T>` 经常一起使用。",
    "表情 😀🎉🚀 不应该把行高撑得离谱。",
    "A\nB\nC",
    "```not code``` but `inline` yes",
    "Mention @a and @b and @c in a row.",
    "`one` `two` `three` `four` `five`",
    "The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog.",
    "短路求值：`false && expensive()` 不会调用右侧。",
    "Wrap `very_long_function_name_that_exceeds_line_width` whole.",
    "User says: `ok`. Agent replies: `ack`.",
    "中文`code`英文`代码`混合。",
];

const MAX_WIDTH: f32 = 480.0;

fn sample_message(content: &str) -> Message {
    let mut msg = Message {
        role: Role::Agent,
        content: content.to_string(),
        blocks: Vec::new(),
        timestamp: Instant::now(),
        parsed: Vec::new(),
        cached_height: None,
        is_error: false,
        lines: Vec::new(),
    };
    msg.prepare();
    msg
}

fn run_alignment(content: &str, ctx: &egui::Context, metrics: &EguiFontMetrics) -> f32 {
    let theme = Theme::default();
    let msg = sample_message(content);
    let estimated = estimate_height(&msg, MAX_WIDTH, &theme, metrics);

    // Ensure fonts are loaded before measuring actual render height.
    let _ = ctx.run_ui(egui::RawInput::default(), |_ui| {});

    let mut actual = 0.0f32;
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(MAX_WIDTH, f32::INFINITY),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    actual = message_bubble(
                        ui,
                        &msg,
                        &theme,
                        false,
                        0,
                        &mut None,
                        &mut false,
                        None,
                        Some(metrics),
                    );
                },
            );
        });
    });

    actual - estimated
}

#[test]
fn pretext_estimate_matches_rendered_height_for_agent_text() {
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    let metrics = EguiFontMetrics::new(ctx.clone());

    let mut max_delta = 0.0f32;
    for sample in ALIGNMENT_SAMPLES {
        let delta = run_alignment(sample, &ctx, &metrics);
        let abs = delta.abs();
        assert!(
            abs <= 32.0,
            "sample {:?}: estimated vs actual delta {:.2}px exceeds 32px",
            sample,
            delta
        );
        max_delta = max_delta.max(abs);
    }

    // Guardrail: the worst sample should still be within a single line height.
    assert!(
        max_delta <= 32.0,
        "worst alignment delta {:.2}px exceeds 32px",
        max_delta
    );
}

#[test]
fn pretext_estimate_matches_rendered_height_for_user_text() {
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    let metrics = EguiFontMetrics::new(ctx.clone());
    let theme = Theme::default();

    for sample in ALIGNMENT_SAMPLES {
        let mut msg = sample_message(sample);
        msg.role = Role::User;
        msg.prepare();
        let msg = msg;
        let estimated = estimate_height(&msg, MAX_WIDTH, &theme, &metrics);

        let mut actual = 0.0f32;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(MAX_WIDTH, f32::INFINITY),
                    egui::Layout::top_down(egui::Align::RIGHT),
                    |ui| {
                        actual = message_bubble(
                            ui,
                            &msg,
                            &theme,
                            false,
                            0,
                            &mut None,
                            &mut false,
                            None,
                            Some(&metrics),
                        );
                    },
                );
            });
        });

        let delta = (actual - estimated).abs();
        assert!(
            delta <= 48.0,
            "user sample {:?}: estimated vs actual delta {:.2}px exceeds 48px",
            sample,
            delta
        );
    }
}

#[test]
#[ignore = "release-mode performance benchmark; run with cargo test -p clarity-egui --bin clarity-egui --release -- --ignored"]
fn pretext_message_list_performance_1000() {
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    let metrics = EguiFontMetrics::new(ctx.clone());
    let theme = Theme::default();

    let messages: Vec<Message> = ALIGNMENT_SAMPLES
        .iter()
        .cycle()
        .take(1000)
        .enumerate()
        .map(|(i, sample)| {
            let mut msg = sample_message(sample);
            if i % 2 == 1 {
                msg.role = Role::User;
                msg.prepare();
            }
            msg
        })
        .collect();

    let estimate_start = Instant::now();
    let mut total_estimated = 0.0f32;
    for msg in &messages {
        total_estimated += estimate_height(msg, MAX_WIDTH, &theme, &metrics);
    }
    let estimate_elapsed = estimate_start.elapsed();

    let render_start = Instant::now();
    let mut total_rendered = 0.0f32;
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            for (i, msg) in messages.iter().enumerate() {
                ui.allocate_ui_with_layout(
                    egui::vec2(MAX_WIDTH, f32::INFINITY),
                    if msg.role == Role::User {
                        egui::Layout::top_down(egui::Align::RIGHT)
                    } else {
                        egui::Layout::top_down(egui::Align::LEFT)
                    },
                    |ui| {
                        total_rendered += message_bubble(
                            ui,
                            msg,
                            &theme,
                            false,
                            i,
                            &mut None,
                            &mut false,
                            None,
                            Some(&metrics),
                        );
                    },
                );
            }
        });
    });
    let render_elapsed = render_start.elapsed();

    eprintln!(
        "pretext 1000 messages: estimate={:.2?} ({:.2?}/msg), render={:.2?} ({:.2?}/msg), total_estimated={:.1}px, total_rendered={:.1}px",
        estimate_elapsed,
        estimate_elapsed.div_f64(1000.0),
        render_elapsed,
        render_elapsed.div_f64(1000.0),
        total_estimated,
        total_rendered
    );

    // Sanity: estimated and rendered totals should be within 5% of each other.
    let delta = (total_rendered - total_estimated).abs();
    let ratio = delta / total_estimated.max(1.0);
    assert!(
        ratio <= 0.05,
        "aggregate height delta ratio {:.2}% exceeds 5%",
        ratio * 100.0
    );
}

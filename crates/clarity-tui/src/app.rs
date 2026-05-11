use crate::async_job::ToolCallJob;
use crate::diff::compute_diff;
use crate::events::ToolCallInfo;
use crate::popup::{EventState, Popup};
use crate::popups::{diff_popup::DiffPopup, HelpPopup, ToolResultPopup};
use anyhow::Result;
use chrono::Local;
use clarity_core::agent::{Agent, AgentController, Op};
use clarity_core::approval::ApprovalMode;
use clarity_core::background::BackgroundTaskManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::commands::{build_default_registry, CommandRegistry};
use crate::events::Event;
use crate::widgets::input_pane::InputPane;
use crate::wire_adapter::spawn_wire_adapter;

#[derive(Debug, Clone)]
pub struct GenerationMetrics {
    pub start_time: std::time::Instant,
    pub first_token_time: Option<std::time::Instant>,
    pub total_chars: usize,
}

/// 消息类型
#[derive(Clone, Debug, PartialEq)]
pub enum MessageType {
    User,
    Assistant,
    System,
    ToolCall,
}

/// 聊天消息
#[derive(Clone, Debug)]
pub struct Message {
    pub content: String,
    pub msg_type: MessageType,
    pub timestamp: String,
    pub is_streaming: bool,
}

impl Message {
    pub fn new(content: impl Into<String>, msg_type: MessageType) -> Self {
        Self {
            content: content.into(),
            msg_type,
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            is_streaming: false,
        }
    }

    pub fn streaming(mut self) -> Self {
        self.is_streaming = true;
        self
    }
}

/// 应用模式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppMode {
    Normal,
    Input,
}

/// TUI 应用状态机。
///
/// 持有聊天历史、输入框、Agent 实例、事件发送器与命令注册表，
/// 负责按键路由、命令解析、滚动控制与生成生命周期管理。
pub struct App {
    /// 聊天历史
    pub messages: Vec<Message>,
    /// 输入框组件
    pub input_pane: InputPane,
    /// 是否运行中
    pub running: bool,
    /// Whether the underlying agent is currently running a turn.
    /// Query via `is_generating()` which reads from `agent.state()`.
    /// 当前模型名称
    pub model_name: String,
    /// 会话ID
    pub session_id: String,
    /// 滚动偏移（以行为单位）
    pub scroll_offset: usize,
    /// 窗口大小
    pub terminal_size: (u16, u16),
    /// 输入框高度
    pub input_height: u16,
    /// 当前模式
    pub mode: AppMode,
    /// 当前弹窗
    pub popup: Option<Box<dyn Popup>>,
    /// 后台任务（工具调用）
    #[allow(dead_code)]
    pub async_job: ToolCallJob,
    /// Agent 实例
    pub(crate) agent: Arc<Agent>,
    /// 事件发送器（用于后台任务发送事件）
    event_tx: Option<UnboundedSender<Event>>,
    /// AgentController 操作发送器
    pub(crate) controller_tx: Option<UnboundedSender<Op>>,
    /// 命令注册表
    pub registry: CommandRegistry,
    /// 生成指标
    pub generation_metrics: Option<GenerationMetrics>,
    /// 命令补全状态
    complete_last_prefix: String,
    complete_last_index: usize,
    /// Session token usage
    pub session_usage: Option<(u32, u32, u32)>,
    /// Skill registry for listing and selection
    pub skill_registry: Option<clarity_core::skills::SkillRegistry>,
    /// File-system watcher for live skill reloading.
    #[allow(dead_code)]
    pub skill_watcher: Option<clarity_core::skills::SkillWatcher>,
    /// Background task manager (shared with Gateway if running)
    pub task_manager: Option<Arc<BackgroundTaskManager>>,
    /// Most recently generated plan, awaiting user confirmation.
    pub pending_plan: Option<clarity_core::agent::Plan>,
    /// Settings ViewModel (populated when entering settings mode).
    #[allow(dead_code)]
    pub settings_vm: Option<clarity_core::view_models::settings::SettingsViewModel>,
    /// Whether the TUI is in settings display mode.
    pub settings_mode: bool,
    /// Cached view commands received from the wire view channel.
    ///
    /// **Deprecated by ADR-006**: scheduled for migration to
    /// `clarity-frontend-ir` in Phase D.
    #[allow(deprecated)]
    pub cached_view_commands: Vec<clarity_wire::ViewCommand>,
}

impl App {
    /// 创建新的 TUI 应用实例。
    ///
    /// 初始化聊天历史（含系统欢迎语）、输入框、默认命令注册表，
    /// 并绑定 Agent 与可选的后台任务管理器。
    pub fn new(
        agent: Arc<Agent>,
        model_name: impl Into<String>,
        task_manager: Option<Arc<BackgroundTaskManager>>,
    ) -> Self {
        let model_name = model_name.into();

        Self {
            messages: vec![Message::new(
                "欢迎使用 Clarity! 输入 /help 查看可用命令。",
                MessageType::System,
            )],
            input_pane: InputPane::new(),
            running: true,

            model_name,
            session_id: format!("session_{}", Local::now().timestamp()),
            scroll_offset: 0,
            terminal_size: (80, 24),
            input_height: 3,
            mode: AppMode::Input,
            popup: None,
            async_job: ToolCallJob::new(),
            agent,
            event_tx: None,
            controller_tx: None,
            registry: build_default_registry(),
            generation_metrics: None,
            complete_last_prefix: String::new(),
            complete_last_index: 0,
            session_usage: None,
            skill_registry: None,
            skill_watcher: None,
            task_manager,
            pending_plan: None,
            settings_vm: None,
            settings_mode: false,
            cached_view_commands: Vec::new(),
        }
    }

    /// Whether the underlying agent is currently running a turn.
    pub fn is_generating(&self) -> bool {
        self.agent.is_running()
    }

    /// Set the agent's approval mode at runtime.
    pub fn set_approval_mode(&self, mode: ApprovalMode) {
        self.agent.set_approval_mode(mode);
    }

    /// Get the agent's current approval mode.
    #[allow(dead_code)]
    pub fn approval_mode(&self) -> ApprovalMode {
        self.agent.approval_mode()
    }

    /// 向后兼容的 input 访问器
    #[allow(dead_code)]
    pub fn input(&self) -> &str {
        self.input_pane.input()
    }

    /// 向后兼容的光标位置访问器
    #[allow(dead_code)]
    pub fn cursor_position(&self) -> usize {
        self.input_pane.cursor_position()
    }

    /// 设置事件发送器，并启动 Wire 适配器与 AgentController。
    ///
    /// 此方法会创建一个 `clarity_wire::Wire`，将 UI 端连接到事件通道，
    /// 同时启动 `AgentController` 在后台运行 Agent 循环。
    pub fn set_event_sender(&mut self, tx: UnboundedSender<Event>) {
        self.event_tx = Some(tx.clone());
        let wire = std::sync::Arc::new(clarity_wire::Wire::new());
        let agent = (*self.agent).clone().with_wire(wire.clone());
        spawn_wire_adapter(wire.ui_side(false), tx.clone());
        // ADR-006: view channel scheduled for removal in 0.4.0; tui will
        // migrate to local SettingsViewModel::commands() in Phase D.
        #[allow(deprecated)]
        crate::wire_adapter::spawn_wire_view_adapter(wire.ui_view_side(), tx);
        let (controller, controller_tx) = AgentController::new_with_sender(agent);
        tokio::spawn(controller.run());
        self.controller_tx = Some(controller_tx);
    }

    /// 处理终端按键事件。
    ///
    /// 全局快捷键（Ctrl+C 停止生成、Ctrl+D 退出）优先处理，
    /// 随后将事件路由给当前弹窗，最后根据 `AppMode` 分发给 Normal 或 Input 模式处理。
    ///
    /// 返回 `Ok(true)` 表示继续运行，`Ok(false)` 表示退出应用。
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // 全局快捷键（Ctrl+C / Ctrl+D）
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => {
                    if self.is_generating() {
                        self.stop_generation();
                    } else {
                        // Ctrl+C when idle -> switch to Normal mode instead of quitting
                        self.mode = AppMode::Normal;
                    }
                    return Ok(true);
                }
                KeyCode::Char('d') => {
                    return Ok(false);
                }
                _ => {}
            }
        }

        // 优先将事件路由给弹窗
        if let Some(ref mut popup) = self.popup {
            let event = crossterm::event::Event::Key(key);
            match popup.handle_event(event) {
                EventState::Consumed => {
                    if popup.is_done() {
                        self.popup = None;
                    }
                    return Ok(true);
                }
                EventState::NotConsumed => {}
            }
        }

        // 过滤 key release/repeat，只处理 press（避免某些终端重复触发）
        if key.kind != crossterm::event::KeyEventKind::Press {
            return Ok(true);
        }

        // Settings 模式优先拦截：Esc 退出，其余忽略
        if self.settings_mode {
            if key.code == KeyCode::Esc {
                self.settings_mode = false;
                self.cached_view_commands.clear();
            }
            return Ok(true);
        }

        match self.mode {
            AppMode::Normal => match key.code {
                KeyCode::Char('q') => return Ok(false),
                KeyCode::Char('?') => {
                    self.popup = Some(Box::new(HelpPopup::new()));
                }
                KeyCode::Char('i') | KeyCode::Enter => {
                    self.mode = AppMode::Input;
                }
                KeyCode::Char('j') | KeyCode::Down => self.scroll_down(),
                KeyCode::Char('k') | KeyCode::Up => self.scroll_up(),
                KeyCode::Char('G') => {
                    let total = self.total_content_lines();
                    let visible = self.visible_chat_height();
                    self.scroll_offset = total.saturating_sub(visible);
                }
                KeyCode::Char('g') => {
                    self.scroll_offset = 0;
                }
                KeyCode::Backspace | KeyCode::Delete => {
                    self.input_pane.delete_char();
                }
                _ => {}
            },
            AppMode::Input => match key.code {
                KeyCode::Esc => {
                    self.mode = AppMode::Normal;
                }
                KeyCode::Enter => {
                    self.submit_message().await?;
                }
                KeyCode::Backspace => {
                    self.input_pane.delete_char();
                }
                KeyCode::Delete => {
                    self.input_pane.delete_char_forward();
                }
                KeyCode::Left => {
                    self.input_pane.move_cursor_left();
                }
                KeyCode::Right => {
                    self.input_pane.move_cursor_right();
                }
                KeyCode::Home => {
                    self.input_pane.set_cursor_position(0);
                }
                KeyCode::End => {
                    let len = self.input_pane.input().chars().count();
                    self.input_pane.set_cursor_position(len);
                }
                KeyCode::Up => {
                    self.input_pane.history_prev();
                }
                KeyCode::Down => {
                    self.input_pane.history_next();
                }
                KeyCode::Tab => {
                    self.complete_command();
                }
                KeyCode::Char(c) => {
                    self.input_pane.insert_char(c);
                }
                _ => {}
            },
        }

        Ok(true)
    }

    /// Approximate total number of rendered content lines.
    fn total_content_lines(&self) -> usize {
        let mut total = 0usize;
        for msg in &self.messages {
            // header line(s)
            match msg.msg_type {
                MessageType::User | MessageType::Assistant => total += 1,
                MessageType::ToolCall => total += 1,
                MessageType::System => {}
            }
            // content lines
            let content_lines = msg.content.lines().count();
            total += content_lines;
            // streaming cursor indicator
            if msg.is_streaming && matches!(msg.msg_type, MessageType::Assistant) {
                total += 1;
            }
            // spacer between messages
            total += 1;
        }
        total
    }

    /// 可见聊天区域高度
    fn visible_chat_height(&self) -> usize {
        // 状态栏 1 行 + 输入框 input_height 行 + 命令栏 1 行
        self.terminal_size
            .1
            .saturating_sub(1 + self.input_height + 1) as usize
    }

    /// 滚动上（按行）
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// 滚动下（按行）
    pub fn scroll_down(&mut self) {
        let total = self.total_content_lines();
        let visible = self.visible_chat_height();
        let max_scroll = total.saturating_sub(visible);
        self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
    }

    /// 提交消息
    async fn submit_message(&mut self) -> Result<()> {
        let content = self.input_pane.input().trim().to_string();
        if content.is_empty() {
            return Ok(());
        }

        self.input_pane.clear();

        if content.starts_with('/') {
            self.handle_command(&content).await;
            return Ok(());
        }

        self.messages
            .push(Message::new(&content, MessageType::User));
        self.start_generation(content).await;

        Ok(())
    }

    /// 处理命令
    async fn handle_command(&mut self, cmd: &str) {
        let trimmed = cmd.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        // /task subcommands need async BackgroundTaskManager access
        if parts[0] == "/task" {
            self.handle_task_command(&parts[1..]).await;
            return;
        }

        // /plan needs async LLM access
        if parts[0] == "/plan" {
            self.handle_plan_command(&parts[1..]).await;
            return;
        }

        // /execute runs the pending plan
        if parts[0] == "/execute" {
            self.handle_execute_command().await;
            return;
        }

        // /parallel runs multiple subagents concurrently
        if parts[0] == "/parallel" {
            self.handle_parallel_command(&parts[1..]).await;
            return;
        }

        if let Some(handler) = self.registry.get(parts[0]) {
            handler.execute(self, &parts[1..]);
        } else {
            self.messages.push(Message::new(
                format!("未知命令: {}。输入 /help 查看可用命令。", parts[0]),
                MessageType::System,
            ));
        }
    }

    async fn handle_task_command(&mut self, args: &[&str]) {
        match args.first().copied() {
            Some("list") | None => match &self.task_manager {
                Some(tm) => match tm.list().await {
                    Ok(tasks) => {
                        if tasks.is_empty() {
                            self.messages
                                .push(Message::new("暂无后台任务。", MessageType::System));
                        } else {
                            let mut lines = vec!["后台任务列表:".to_string()];
                            for t in tasks {
                                lines
                                    .push(format!("  {} | {:?} | {}", t.id, t.status, t.spec.name));
                            }
                            self.messages
                                .push(Message::new(lines.join("\n"), MessageType::System));
                        }
                    }
                    Err(e) => {
                        self.messages.push(Message::new(
                            format!("获取任务列表失败: {}", e),
                            MessageType::System,
                        ));
                    }
                },
                None => {
                    self.messages.push(Message::new(
                        "后台任务管理器未初始化。",
                        MessageType::System,
                    ));
                }
            },
            Some("status") => {
                if args.len() < 2 {
                    self.messages.push(Message::new(
                        "用法: /task status <task-id>",
                        MessageType::System,
                    ));
                    return;
                }
                let id = args[1].to_string();
                match &self.task_manager {
                    Some(tm) => match tm.status(&id).await {
                        Ok(status) => {
                            self.messages.push(Message::new(
                                format!("任务 {} 状态: {:?}", id, status),
                                MessageType::System,
                            ));
                        }
                        Err(e) => {
                            self.messages.push(Message::new(
                                format!("查询任务状态失败: {}", e),
                                MessageType::System,
                            ));
                        }
                    },
                    None => {
                        self.messages.push(Message::new(
                            "后台任务管理器未初始化。",
                            MessageType::System,
                        ));
                    }
                }
            }
            Some("cancel") => {
                if args.len() < 2 {
                    self.messages.push(Message::new(
                        "用法: /task cancel <task-id>",
                        MessageType::System,
                    ));
                    return;
                }
                let id = args[1].to_string();
                match &self.task_manager {
                    Some(tm) => match tm.cancel(&id).await {
                        Ok(()) => {
                            self.messages.push(Message::new(
                                format!("已取消任务 {}", id),
                                MessageType::System,
                            ));
                        }
                        Err(e) => {
                            self.messages.push(Message::new(
                                format!("取消任务失败: {}", e),
                                MessageType::System,
                            ));
                        }
                    },
                    None => {
                        self.messages.push(Message::new(
                            "后台任务管理器未初始化。",
                            MessageType::System,
                        ));
                    }
                }
            }
            Some("spawn") => {
                if args.len() < 3 {
                    self.messages.push(Message::new(
                        "用法: /task spawn <name> <prompt...>",
                        MessageType::System,
                    ));
                    return;
                }
                let name = args[1].to_string();
                let prompt = args[2..].join(" ");
                match &self.task_manager {
                    Some(tm) => {
                        let spec = clarity_core::background::TaskSpec::new(name, prompt)
                            .with_agent_type("default")
                            .with_max_iterations(10);
                        match tm.spawn_agent(spec).await {
                            Ok(id) => {
                                self.messages.push(Message::new(
                                    format!("已创建后台任务 {}", id),
                                    MessageType::System,
                                ));
                            }
                            Err(e) => {
                                self.messages.push(Message::new(
                                    format!("创建任务失败: {}", e),
                                    MessageType::System,
                                ));
                            }
                        }
                    }
                    None => {
                        self.messages.push(Message::new(
                            "后台任务管理器未初始化。",
                            MessageType::System,
                        ));
                    }
                }
            }
            Some(sub) => {
                self.messages.push(Message::new(
                    format!(
                        "未知 /task 子命令: {}。可用: list, status, cancel, spawn",
                        sub
                    ),
                    MessageType::System,
                ));
            }
        }
    }

    async fn handle_plan_command(&mut self, args: &[&str]) {
        if args.is_empty() {
            self.messages.push(Message::new(
                "用法: /plan <query> — 让 Agent 生成执行计划",
                MessageType::System,
            ));
            return;
        }
        if self.is_generating() {
            self.messages.push(Message::new(
                "Agent 正在运行中，请等待当前任务完成后再使用 /plan。",
                MessageType::System,
            ));
            return;
        }
        let query = args.join(" ");
        self.messages.push(Message::new(
            format!("📝 正在生成计划: {}...", query),
            MessageType::System,
        ));
        match self.agent.plan(&query).await {
            Ok(plan) => {
                let mut lines = vec![format!("📋 计划: {}", plan.title)];
                if plan.is_empty() {
                    lines.push("无需执行任何步骤。".to_string());
                } else {
                    for step in &plan.steps {
                        lines.push(format!(
                            "  {}. {} (`{}`)",
                            step.id, step.description, step.tool_name
                        ));
                    }
                    lines.push(String::new());
                    lines.push("输入 /execute 执行此计划，或继续对话。".to_string());
                }
                self.pending_plan = Some(plan);
                self.messages
                    .push(Message::new(lines.join("\n"), MessageType::System));
            }
            Err(e) => {
                self.messages.push(Message::new(
                    format!("生成计划失败: {}", e),
                    MessageType::System,
                ));
            }
        }
    }

    async fn handle_parallel_command(&mut self, args: &[&str]) {
        if args.is_empty() {
            self.messages.push(Message::new(
                "用法: /parallel <type>:<prompt> [| <type>:<prompt>...]\n示例: /parallel coder:实现斐波那契函数 | explore:查找所有测试文件",
                MessageType::System,
            ));
            return;
        }
        if self.is_generating() {
            self.messages.push(Message::new(
                "Agent 正在运行中，请等待当前任务完成后再使用 /parallel。",
                MessageType::System,
            ));
            return;
        }

        let raw = args.join(" ");
        let parsed = crate::parse::parse_parallel_args(&raw);
        if parsed.is_empty() {
            self.messages
                .push(Message::new("未指定任何子代理任务。", MessageType::System));
            return;
        }

        let specs: Vec<clarity_contract::subagent::RunSpec> = parsed
            .into_iter()
            .map(|(agent_type, prompt)| {
                clarity_contract::subagent::RunSpec::new(
                    format!("parallel-{}", &agent_type),
                    prompt,
                )
                .with_type(&agent_type)
            })
            .collect();

        self.messages.push(Message::new(
            format!("🚀 启动并行执行 ({} 个子代理)...", specs.len()),
            MessageType::System,
        ));

        let config = clarity_contract::subagent::ParallelConfig::new()
            .with_max_concurrency(specs.len().min(4));

        match self.agent.run_parallel(specs, config, None).await {
            Ok(result) => {
                let elapsed_s = result.total_elapsed_ms as f64 / 1000.0;
                let total = result.results.len() + result.failures.len();
                let mut lines = vec![
                    format!("┌─────────────────────────────────────────────┐"),
                    format!(
                        "│ 🚀 并行执行完成  {} 个任务  {:.1}s          │",
                        total, elapsed_s
                    ),
                    format!(
                        "│ 成功率: {:>3.0}%                                  │",
                        result.success_rate() * 100.0
                    ),
                    format!("└─────────────────────────────────────────────┘"),
                ];

                if !result.results.is_empty() {
                    lines.push(String::new());
                    lines.push("✅ 成功结果:".to_string());
                    lines.push(format!("  {:<10} {:<10} {}", "代理ID", "类型", "摘要"));
                    lines.push(format!("  {:<10} {:<10} {}", "──────", "────", "────"));
                    for r in &result.results {
                        let id = if r.agent_id.len() > 8 {
                            &r.agent_id[..8]
                        } else {
                            &r.agent_id
                        };
                        let summary = r.summary.lines().next().unwrap_or("No summary");
                        let summary = if summary.len() > 36 {
                            &summary[..36]
                        } else {
                            summary
                        };
                        lines.push(format!("  {:<10} {:<10} {}", id, r.agent_type, summary));
                    }
                }

                if !result.failures.is_empty() {
                    lines.push(String::new());
                    lines.push("❌ 失败任务:".to_string());
                    for (id, err) in &result.failures {
                        lines.push(format!("  • {}: {}", id, err.lines().next().unwrap_or("")));
                    }
                }

                if let Some(ref summary) = result.aggregated_summary {
                    lines.push(String::new());
                    lines.push("📋 聚合摘要:".to_string());
                    for line in summary.lines() {
                        lines.push(format!("  {}", line));
                    }
                }

                self.messages
                    .push(Message::new(lines.join("\n"), MessageType::System));
            }
            Err(e) => {
                self.messages.push(Message::new(
                    format!("并行执行失败: {}", e),
                    MessageType::System,
                ));
            }
        }
    }

    async fn handle_execute_command(&mut self) {
        match self.pending_plan.take() {
            Some(plan) => {
                if self.is_generating() {
                    self.messages.push(Message::new(
                        "Agent 正在运行中，请等待当前任务完成后再执行。",
                        MessageType::System,
                    ));
                    self.pending_plan = Some(plan);
                    return;
                }
                self.messages.push(Message::new(
                    format!("▶️ 开始执行计划: {} ({} 步)", plan.title, plan.len()),
                    MessageType::System,
                ));
                match self.agent.execute_plan(&plan).await {
                    Ok(results) => {
                        let mut lines = vec!["✅ 计划执行完成:".to_string()];
                        for r in &results {
                            let icon = if r.success { "✓" } else { "✗" };
                            lines.push(format!("  {} {}: {}", icon, r.step_id, r.output));
                        }
                        self.messages
                            .push(Message::new(lines.join("\n"), MessageType::System));
                    }
                    Err(e) => {
                        self.messages.push(Message::new(
                            format!("执行计划失败: {}", e),
                            MessageType::System,
                        ));
                    }
                }
            }
            None => {
                self.messages.push(Message::new(
                    "没有待执行的计划。先用 /plan <query> 生成计划。",
                    MessageType::System,
                ));
            }
        }
    }

    /// 开始生成响应（统一通过 AgentController 发送 UserTurn，流式事件由
    /// clarity-wire → wire_adapter → Event 路径接收）。
    async fn start_generation(&mut self, user_input: String) {
        self.generation_metrics = Some(GenerationMetrics {
            start_time: std::time::Instant::now(),
            first_token_time: None,
            total_chars: 0,
        });
        self.messages
            .push(Message::new("", MessageType::Assistant).streaming());

        if let Some(ref controller_tx) = self.controller_tx {
            let _ = controller_tx.send(Op::UserTurn(user_input));
        }
    }

    /// 停止生成
    pub fn stop_generation(&mut self) {
        if self.is_generating() {
            if let Some(ref controller_tx) = self.controller_tx {
                let _ = controller_tx.send(Op::Interrupt);
            }
            self.generation_metrics = None;
            if let Some(last) = self.messages.last_mut() {
                last.is_streaming = false;
            }
            self.messages
                .push(Message::new("⏹️ 生成已停止。", MessageType::System));
        }
    }

    /// Set the active skill on the underlying agent.
    /// Kept for backward compatibility; prefer `skill_registry().toggle_active()`.
    #[allow(dead_code, deprecated)]
    pub fn set_active_skill(&self, skill_id: Option<String>) {
        self.agent.set_active_skill(skill_id);
    }

    /// Get the currently active skill id from the underlying agent.
    /// Kept for backward compatibility; prefer `skill_registry().active_ids()`.
    #[allow(dead_code, deprecated)]
    pub fn active_skill(&self) -> Option<String> {
        self.agent.active_skill()
    }

    /// 完成生成
    pub fn finish_generation(&mut self) {
        self.generation_metrics = None;
        if let Some(last) = self.messages.last_mut() {
            last.is_streaming = false;
        }
    }

    /// 处理错误
    pub fn handle_error(&mut self, error: String) {
        self.generation_metrics = None;
        if let Some(last) = self.messages.last_mut() {
            last.is_streaming = false;
        }
        self.messages.push(Message::new(error, MessageType::System));
    }

    /// 处理流式响应块
    pub fn handle_stream_chunk(&mut self, chunk: String) {
        if let Some(ref mut metrics) = self.generation_metrics {
            if metrics.first_token_time.is_none() && !chunk.is_empty() {
                metrics.first_token_time = Some(std::time::Instant::now());
            }
            metrics.total_chars += chunk.chars().count();
        }
        if let Some(last) = self.messages.last_mut() {
            if matches!(last.msg_type, MessageType::Assistant) && last.is_streaming {
                last.content.push_str(&chunk);
            }
        }
    }

    /// 处理工具调用
    pub fn handle_tool_call(&mut self, tool: ToolCallInfo) {
        let text = if tool.params.is_empty() {
            format!("🔧 调用工具: {}", tool.name)
        } else {
            format!("🔧 调用工具: {} 参数: {}", tool.name, tool.params)
        };
        self.messages
            .push(Message::new(text, MessageType::ToolCall));
    }

    /// 处理工具结果
    pub fn handle_tool_result(&mut self, tool: ToolCallInfo) {
        let text = format!("✅ 工具结果: {}", tool.params);
        self.messages
            .push(Message::new(text, MessageType::ToolCall));

        let has_diff = serde_json::from_str::<serde_json::Value>(&tool.params)
            .ok()
            .and_then(|json| json.get("_diff_preview").cloned())
            .is_some();

        if has_diff {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&tool.params) {
                if let Some(diff_preview) = json.get("_diff_preview") {
                    let path = json.get("path").and_then(|v| v.as_str());

                    // Sprint 11 Phase B: unified diff patch string
                    if let Some(patch) = diff_preview.as_str() {
                        if let Some(path) = path {
                            self.popup = Some(Box::new(DiffPopup::from_patch(
                                path.to_string(),
                                patch.to_string(),
                            )));
                            return;
                        }
                    }

                    // Legacy: {old, new} object (backward compatible)
                    if let (Some(path), Some(old), Some(new)) = (
                        path,
                        diff_preview.get("old").and_then(|v| v.as_str()),
                        diff_preview.get("new").and_then(|v| v.as_str()),
                    ) {
                        self.popup = Some(Box::new(DiffPopup::new(
                            path.to_string(),
                            compute_diff(old, new),
                        )));
                        return;
                    }
                }
            }
        }

        self.popup = Some(Box::new(ToolResultPopup::new(
            format!("Tool: {}", tool.name),
            tool.params,
        )));
    }

    /// 命令补全（Tab 键）
    fn complete_command(&mut self) {
        let input = self.input_pane.input();
        if !input.starts_with('/') {
            return;
        }
        let prefix = input.to_lowercase();
        let candidates: Vec<&str> = self
            .registry
            .names()
            .into_iter()
            .filter(|name| name.to_lowercase().starts_with(&prefix))
            .collect();
        if candidates.is_empty() {
            return;
        }
        // Cycle through candidates on repeated Tab presses
        let (next_cmd, _next_idx) = if self.complete_last_prefix != prefix {
            self.complete_last_prefix = prefix.clone();
            self.complete_last_index = 0;
            (candidates[0], 0)
        } else {
            let idx = (self.complete_last_index + 1) % candidates.len();
            self.complete_last_index = idx;
            (candidates[idx], idx)
        };
        // Replace input with the selected command + trailing space
        self.input_pane.clear();
        for c in next_cmd.chars() {
            self.input_pane.insert_char(c);
        }
        self.input_pane.insert_char(' ');
    }

    /// 时钟滴答
    pub fn on_tick(&mut self) {
        // 可以在这里处理定时任务
    }

    /// Handle usage update from agent
    pub fn handle_usage(&mut self, prompt_tokens: u32, completion_tokens: u32, total_tokens: u32) {
        self.session_usage = Some((prompt_tokens, completion_tokens, total_tokens));
    }

    /// 窗口大小变化
    pub fn on_resize(&mut self, width: u16, height: u16) {
        self.terminal_size = (width, height);
    }
}

impl Default for App {
    fn default() -> Self {
        let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
        let config = clarity_core::agent::AgentConfig::default();
        let agent = Arc::new(Agent::with_config(registry, config));
        Self::new(agent, "default", None)
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::events::ToolStatus;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn test_app() -> App {
        App::default()
    }

    #[test]
    fn test_app_new_initial_state() {
        let app = test_app();
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(app.messages[0].msg_type, MessageType::System));
        assert!(!app.is_generating());
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.mode, AppMode::Input);
        assert!(app.popup.is_none());
    }

    #[test]
    fn test_add_message() {
        let mut app = test_app();
        app.messages.push(Message::new("hello", MessageType::User));
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages.last().unwrap().content, "hello");
    }

    #[test]
    fn test_scroll_up_down() {
        let mut app = test_app();
        app.messages
            .push(Message::new("a\nb\nc\nd\ne", MessageType::User));
        app.terminal_size = (80, 10);
        app.input_height = 3;

        let initial_offset = app.scroll_offset;
        app.scroll_down();
        assert!(app.scroll_offset > initial_offset);

        let after_down = app.scroll_offset;
        app.scroll_up();
        assert_eq!(app.scroll_offset, after_down - 1);
    }

    #[test]
    fn test_scroll_up_does_not_underflow() {
        let mut app = test_app();
        app.scroll_offset = 0;
        app.scroll_up();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_handle_tool_call() {
        let mut app = test_app();
        app.handle_tool_call(ToolCallInfo {
            name: "read_file".to_string(),
            params: "{\"path\":\"/tmp\"}".to_string(),
            status: ToolStatus::Running,
        });
        assert_eq!(app.messages.len(), 2);
        let last = app.messages.last().unwrap();
        assert!(matches!(last.msg_type, MessageType::ToolCall));
        assert!(last.content.contains("read_file"));
    }

    #[test]
    fn test_handle_tool_result() {
        let mut app = test_app();
        app.handle_tool_result(ToolCallInfo {
            name: "result".to_string(),
            params: "ok".to_string(),
            status: ToolStatus::Success,
        });
        let last = app.messages.last().unwrap();
        assert!(last.content.contains("ok"));
        assert!(app.popup.is_some());
    }

    #[test]
    fn test_finish_generation() {
        let mut app = test_app();
        app.messages
            .push(Message::new("", MessageType::Assistant).streaming());
        app.finish_generation();
        assert!(!app.is_generating());
        assert!(!app.messages.last().unwrap().is_streaming);
    }

    #[test]
    fn test_handle_error() {
        let mut app = test_app();
        app.messages
            .push(Message::new("", MessageType::Assistant).streaming());
        app.handle_error("boom".to_string());
        assert!(!app.is_generating());
        let last = app.messages.last().unwrap();
        assert_eq!(last.content, "boom");
        assert!(matches!(last.msg_type, MessageType::System));
    }

    #[tokio::test]
    async fn test_handle_key_exit_ctrl_d() {
        let mut app = test_app();
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let keep = app.handle_key(key).await.unwrap();
        assert!(!keep);
    }

    #[tokio::test]
    async fn test_app_event_routing_to_popup_consumed() {
        let mut app = test_app();
        app.popup = Some(Box::new(HelpPopup::new()));
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
        let keep = app.handle_key(key).await.unwrap();
        assert!(keep);
        assert!(app.popup.is_none());
    }

    #[tokio::test]
    async fn test_app_open_help_popup_in_normal_mode() {
        let mut app = test_app();
        app.mode = AppMode::Normal;
        let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty());
        let keep = app.handle_key(key).await.unwrap();
        assert!(keep);
        assert!(app.popup.is_some());
    }

    #[tokio::test]
    async fn test_command_registry_model() {
        let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
        let config = clarity_core::agent::AgentConfig::default();
        let agent = Arc::new(Agent::with_config(registry, config));
        let mut app = App::new(agent, "default", None);
        app.handle_command("/model gpt-4").await;
        assert_eq!(app.model_name, "gpt-4");
    }

    #[test]
    fn test_generation_metrics() {
        let mut app = test_app();
        app.start_generation_sync();
        assert!(app.generation_metrics.is_some());
        app.handle_stream_chunk("hello".to_string());
        let metrics = app.generation_metrics.as_ref().unwrap();
        assert_eq!(metrics.total_chars, 5);
        assert!(metrics.first_token_time.is_some());
    }

    // =========================================================================
    // End-to-end render + key flow (TestBackend)
    // =========================================================================

    #[tokio::test]
    async fn test_end_to_end_render_and_key_flow() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut app = test_app();
        app.terminal_size = (40, 12);

        let backend = TestBackend::new(40, 12);
        let mut terminal = Terminal::new(backend).unwrap();

        // 1. Initial render — should show welcome / system message
        terminal.draw(|f| crate::ui::draw(f, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let screen_text: String = buf.content.iter().map(|cell| cell.symbol()).collect();
        assert!(
            screen_text.contains("Clarity") || screen_text.contains("Welcome"),
            "expected welcome text on initial render, got: {}",
            screen_text
        );

        // 2. Type "hi" in Input mode
        for ch in "hi".chars() {
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty());
            let keep = app.handle_key(key).await.unwrap();
            assert!(keep, "app should keep running after typing '{}'", ch);
        }

        // 3. Render after typing — input should appear
        terminal.draw(|f| crate::ui::draw(f, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let screen_text: String = buf.content.iter().map(|cell| cell.symbol()).collect();
        assert!(
            screen_text.contains('h') && screen_text.contains('i'),
            "expected typed text 'hi' on screen, got: {}",
            screen_text
        );

        // 4. Press Esc to switch to Normal mode
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
        let keep = app.handle_key(esc).await.unwrap();
        assert!(keep, "app should keep running after Esc");
        assert_eq!(app.mode, AppMode::Normal, "mode should switch to Normal");

        // 5. Press 'q' to quit
        let quit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty());
        let keep = app.handle_key(quit).await.unwrap();
        assert!(!keep, "app should stop after 'q' in Normal mode");
    }
}

impl App {
    #[cfg(test)]
    fn start_generation_sync(&mut self) {
        self.generation_metrics = Some(GenerationMetrics {
            start_time: std::time::Instant::now(),
            first_token_time: None,
            total_chars: 0,
        });
        self.messages
            .push(Message::new("", MessageType::Assistant).streaming());
    }
}

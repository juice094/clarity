//! Minimal i18n: key = English string, lookup returns translation for the
//! active locale.  Keys that are not found fall back to the key itself
//! (English).
//!
//! USAGE
//! -----
//! ```ignore
//! // In any render function that has access to `app`:
//! let label = app.t("Settings");
//! ui.label(label);
//!
//! // Or, for a static locale (e.g. in `Theme` or `ui/` helpers):
//! let label = Locale::ZhCN.t("Save");
//! ```
//!
//! EXTENDING
//! ---------
//! Add new entries to `ZH_CN` map below.  No need to restart — reload
//! happens on next frame.
//!
//! TODO-WEEK2: Load translation maps from JSON files so locale packs can
//! be added without recompilation.

use std::collections::HashMap;

// ============================================================================
// Locale enum
// ============================================================================

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[allow(dead_code)]
pub enum Locale {
    #[default]
    EnUS,
    ZhCN,
}

impl Locale {
    /// Look up `key` in the current locale's translation map.
    /// Falls back to the key itself (English) when no translation exists.
    pub fn t(self, key: &'static str) -> &'static str {
        match self {
            Locale::EnUS => key,
            Locale::ZhCN => ZH_CN.get(key).copied().unwrap_or(key),
        }
    }

    /// Human-readable label for this locale (e.g. for a settings dropdown).
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Locale::EnUS => "English",
            Locale::ZhCN => "简体中文",
        }
    }
}

// ============================================================================
// Translation maps
// ============================================================================

/// English → Simplified Chinese.
static ZH_CN: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // ── App / sidebar ──
    m.insert("Clarity", "Clarity");
    m.insert("Files", "文件");
    m.insert("Skills", "技能");
    m.insert("Tools", "工具");
    m.insert("Tasks", "任务");
    m.insert("Settings", "设置");
    m.insert("New Background Task", "新建后台任务");
    m.insert("Create Task", "创建任务");
    m.insert("+ Create Task", "+ 创建任务");
    m.insert("Cancel", "取消");
    m.insert("Save", "保存");
    m.insert("Create", "创建");

    // ── Sidebar categories ──
    m.insert("Emotion", "情感");
    m.insert("Knowledge", "知识");
    m.insert("Engineering", "工程");

    // ── Agent status ──
    m.insert("Online", "在线");
    m.insert("Busy", "忙碌");
    m.insert("Offline", "离线");
    m.insert("Unconfigured", "未配置");

    // ── Chat ──
    m.insert("Type a message...", "输入消息…");
    m.insert("Type a message (files attached)...", "输入消息（已附加文件）…");
    m.insert("Local-first AI agent runtime", "本地优先的 AI 代理运行环境");
    m.insert("Preview", "预览");
    m.insert("Configure Settings", "配置设置");
    m.insert("Tool Call Approval", "工具调用审批");
    m.insert("Send message", "发送消息");
    m.insert("Stop generating (Ctrl+C)", "停止生成（Ctrl+C）");

    // ── MCP ──
    m.insert("No MCP servers configured", "未配置 MCP 服务器");
    m.insert("Command:", "命令：");
    m.insert("Args:", "参数：");
    m.insert("Transport:", "传输方式：");

    // ── Empty states ──
    m.insert("No tasks yet", "暂无任务");
    m.insert("No skills found.", "未找到技能。");

    // ── Onboarding ──
    m.insert("Welcome to Clarity", "欢迎使用 Clarity");
    m.insert("Enter API Key (Cloud Provider)", "输入 API Key（云端服务商）");
    m.insert("Download Local Model (~1 GB)", "下载本地模型（约 1 GB）");
    m.insert("Skip for Now", "暂时跳过");
    m.insert("Downloading Local Model", "正在下载本地模型");
    m.insert("Download Complete", "下载完成");
    m.insert("Download Failed", "下载失败");
    m.insert("Start Using Clarity", "开始使用 Clarity");
    m.insert("Try Again", "重试");
    m.insert("Enter API Key Instead", "改用 API Key");

    m
});

use std::sync::LazyLock;

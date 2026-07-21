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
//! NOTE: Future enhancement — load translation maps from JSON files so
//! locale packs can be added without recompilation.

use std::collections::HashMap;

// ============================================================================
// Locale enum
// ============================================================================

/// locale variants.
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

    /// Short persisted code for this locale.
    pub fn as_code(self) -> &'static str {
        match self {
            Locale::EnUS => "en",
            Locale::ZhCN => "zh",
        }
    }

    /// Parse a persisted locale code.
    pub fn from_code(code: &str) -> Self {
        match code.trim().to_lowercase().as_str() {
            "zh" | "zh-cn" | "zh_cn" | "zh-hans" => Locale::ZhCN,
            _ => Locale::EnUS,
        }
    }
}

// ============================================================================
// Translation maps
// ============================================================================

/// English → Simplified Chinese.
static ZH_CN: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // ── App / sidebar / navigation tree ──
    m.insert("Clarity", "Clarity");
    m.insert("Files", "文件");
    m.insert("Skills", "技能");
    m.insert("Tools", "工具");
    m.insert("Tasks", "任务");
    m.insert("Settings", "设置");
    m.insert("New Session", "新建会话");
    m.insert("New Chat", "新建会话");
    m.insert("New session (Ctrl+N)", "新建会话（Ctrl+N）");
    m.insert("New chat (Ctrl+N)", "新建会话（Ctrl+N）");
    m.insert("New task (Ctrl+N)", "新建任务（Ctrl+N）");
    m.insert("New Task", "新建任务");
    m.insert("New cron schedule", "新建定时任务");
    m.insert("Plugins", "插件");
    m.insert("Work", "工作");
    m.insert("Chat", "聊天");
    m.insert("History", "历史会话");
    m.insert("Work Templates", "工作模板");
    m.insert("No bookmarks", "暂无书签");
    m.insert("No templates", "暂无模板");
    m.insert("Add", "添加");
    m.insert("Close", "关闭");
    m.insert("Manage bookmarks", "管理书签");
    m.insert("Manage templates", "管理模板");
    m.insert("Web", "网页");
    m.insert("Docs", "文档");
    m.insert(
        "Type / for plugins, # for context...",
        "输入 / 唤起插件，# 添加上下文...",
    );
    m.insert("GitHub", "GitHub");
    m.insert("Claw", "Claw");
    m.insert("Workspace", "工作空间");
    m.insert("Terminal", "终端");
    m.insert("WebBridge", "网页桥");
    m.insert("No devices", "暂无设备");
    m.insert("Projects", "项目");
    m.insert("No projects yet", "暂无项目");
    m.insert("Archived", "已归档");
    m.insert("Chats", "对话");
    m.insert("No sessions", "暂无会话");
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
    m.insert(
        "Type a message (files attached)...",
        "输入消息（已附加文件）…",
    );
    m.insert(
        "Message queued, will send after current response...",
        "消息已排队，将在当前回复结束后发送…",
    );
    m.insert("Local-first AI agent runtime", "本地优先的 AI 代理运行环境");
    m.insert("Start conversation below", "在下方输入框开始对话");
    m.insert("Code assistant", "代码助手");
    m.insert("Task planning", "任务规划");
    m.insert("Code review", "代码审查");
    m.insert("Preview", "预览");
    m.insert("Configure Settings", "配置设置");
    m.insert("Tool Call Approval", "工具调用审批");
    m.insert("Send message", "发送消息");
    m.insert("Stop generating (Ctrl+C)", "停止生成（Ctrl+C）");

    // ── Bot bar / Right rail ──
    m.insert("Share", "分享");
    m.insert("Console", "控制台");
    m.insert("Knowledge", "知识库");
    m.insert("Remote", "远程");
    m.insert("Local", "本地");
    m.insert("Templates", "模板");
    m.insert("Collapse right rail", "折叠右栏");
    m.insert("Select a panel from the Bot bar", "从 Bot 栏选择一个面板");
    m.insert("Panel", "面板");
    m.insert("Claw remote settings", "Claw 远程设置");
    m.insert("Console / task log", "控制台 / 任务日志");
    m.insert("Files / workspace", "文件 / 工作区");
    m.insert("Knowledge base", "知识库");
    m.insert("Share conversation", "分享会话");
    m.insert("Bundle path", "Bundle 路径");
    m.insert("Load bundle", "加载 Bundle");
    m.insert("Reload", "重新加载");
    m.insert("Search concepts", "搜索概念");
    m.insert("Concepts", "概念");
    m.insert("Type", "类型");
    m.insert("Tags", "标签");
    m.insert("No concepts found", "未找到概念");
    m.insert("Select a concept to view details", "选择一个概念查看详情");
    m.insert("Failed to load bundle", "加载 Bundle 失败");
    m.insert("Loading…", "加载中…");
    m.insert("Path to OKF bundle directory", "OKF Bundle 目录路径");
    m.insert(
        "Enter a bundle path and click Load bundle",
        "输入 Bundle 路径并点击加载",
    );

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
    m.insert(
        "Enter API Key (Cloud Provider)",
        "输入 API Key（云端服务商）",
    );
    m.insert("Download Local Model (~1 GB)", "下载本地模型（约 1 GB）");
    m.insert("Skip for Now", "暂时跳过");
    m.insert("Downloading Local Model", "正在下载本地模型");
    m.insert("Download Complete", "下载完成");
    m.insert("Download Failed", "下载失败");
    m.insert("Start Using Clarity", "开始使用 Clarity");
    m.insert("Try Again", "重试");
    m.insert("Enter API Key Instead", "改用 API Key");
    m.insert("Tools", "工具");
    m.insert("No skills found.", "未找到技能。");
    m.insert(
        "Place .md files in .clarity/skills/ to add skills.",
        "将 .md 文件放入 .clarity/skills/ 目录以添加技能。",
    );
    m.insert("skill(s) loaded", "个技能已加载");
    m.insert("Skill(s)", "技能");
    m.insert("Active tasks:", "活跃任务：");
    m.insert("Category:", "分类：");
    m.insert("No tasks yet", "暂无任务");
    m.insert("Name", "名称");
    m.insert("Description", "描述");
    m.insert("Prompt", "提示词");
    m.insert("URL", "链接");
    m.insert("Priority", "优先级");
    m.insert("Tool:", "工具：");
    m.insert("Arguments:", "参数：");
    m.insert("Preview:", "预览：");
    m.insert("Reject", "拒绝");
    m.insert("Approve", "批准");
    m.insert("Approve for Session", "本次会话批准");
    m.insert("Enabled", "已启用");
    m.insert("Environment:", "环境变量：");
    m.insert("MCP Servers", "MCP 服务器");
    m.insert("Pair", "配对");
    m.insert("Pairing...", "正在配对…");
    m.insert("Waiting for approval", "等待审批");
    m.insert("Paired", "已配对");
    m.insert("Pairing failed", "配对失败");
    m.insert("Compacting conversation history…", "正在压缩对话历史…");
    // ── Settings ──
    m.insert("General", "通用");
    m.insert("Provider", "服务商");
    m.insert("Interface", "界面");
    m.insert("About", "关于");
    m.insert("Claw", "Claw");
    m.insert(
        "Manage OpenClaw Gateway connections",
        "管理 OpenClaw 网关连接",
    );
    m.insert(
        "No OpenClaw connections configured.",
        "未配置 OpenClaw 连接。",
    );
    m.insert("Connection", "连接");
    m.insert("Gateway URL", "网关 URL");
    m.insert("Auth Mode", "认证模式");
    m.insert("Device Token", "设备令牌");
    m.insert("Enabled", "已启用");
    m.insert("Update", "更新");
    m.insert("Theme", "主题");
    m.insert("Language", "语言");
    m.insert("Layout Debug", "布局调试");
    m.insert(
        "Show green/blue/red/yellow layout diagnostics (Ctrl+Shift+L)",
        "显示绿/蓝/红/黄布局诊断覆盖层（Ctrl+Shift+L）",
    );
    m.insert("Zoom", "缩放");
    m.insert(
        "Use Ctrl + +/- to adjust zoom anytime",
        "使用 Ctrl + +/- 随时调整缩放",
    );
    m.insert("Model", "模型");
    m.insert("API Key", "API 密钥");
    m.insert("Approval Mode", "审批模式");
    m.insert("Built-in", "内置");
    m.insert("Custom", "自定义");
    m.insert("Delete", "删除");
    m.insert("+ Add Custom Provider", "+ 添加自定义服务商");
    m.insert("Add Custom Provider", "添加自定义服务商");
    m.insert("Clear Batch Grants", "清除批量授权");
    m.insert("No custom providers configured.", "未配置自定义服务商。");

    // ── Settings: model catalog refresh ──
    m.insert("Refresh Models", "刷新模型");
    m.insert("Refreshing...", "刷新中…");
    m.insert("Refresh model list", "从 API 拉取最新模型列表");
    m.insert("Refresh in progress...", "正在刷新模型列表…");
    m.insert("Retry model refresh", "重试拉取模型列表");
    m.insert(
        "This channel has no public model API",
        "该渠道无公开模型 API",
    );
    m.insert("models", "个模型");
    m.insert("Model refresh failed", "模型列表刷新失败");

    // ── Ops tab ──
    m.insert("Ops Actions", "运维操作");
    m.insert("AI Diagnostics", "AI 问题诊断");
    m.insert("Run self-diagnostic checks", "运行自诊断检查");
    m.insert("Restart Gateway", "重启 Gateway");
    m.insert("Restart local Gateway service", "重启本地 Gateway 服务");
    m.insert("Repair Config", "修复配置");
    m.insert("Auto-repair common config issues", "自动修复常见配置问题");
    m.insert("Open Terminal", "打开终端");
    m.insert("Open system terminal", "打开系统终端");
    m.insert("Data Backup", "数据备份");
    m.insert("Backup current sessions and config", "备份当前会话和配置");
    m.insert("System Status", "系统状态");
    m.insert("View detailed system status", "查看详细系统状态");
    m.insert("Version Info", "版本信息");
    m.insert("Last Backup", "上次备份");
    m.insert("AI diagnostic running…", "AI 诊断运行中…");
    m.insert("Gateway restart request sent", "Gateway 重启请求已发送");
    m.insert("Repairing config…", "配置修复中…");
    m.insert("Data backup complete", "数据备份完成");

    // ── Claw role-context E2EE ──
    m.insert("Role passphrase", "角色口令");
    m.insert(
        "Encrypts role-context events stored by Syncthing",
        "加密 Syncthing 存储的角色上下文事件",
    );
    m.insert(
        "Select a Claw device to set a passphrase",
        "选择一台 Claw 设备以设置口令",
    );
    m.insert("Role", "角色");
    m.insert("Enter passphrase…", "输入口令…");
    m.insert("Apply passphrase", "应用口令");
    m.insert("Passphrase applied", "口令已应用");
    m.insert("Passphrase cleared", "口令已清除");

    m
});

use std::sync::LazyLock;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_codes_roundtrip() {
        assert_eq!(Locale::EnUS.as_code(), "en");
        assert_eq!(Locale::ZhCN.as_code(), "zh");
        assert_eq!(Locale::from_code("en"), Locale::EnUS);
        assert_eq!(Locale::from_code("zh"), Locale::ZhCN);
    }

    #[test]
    fn locale_from_code_is_case_and_dash_tolerant() {
        assert_eq!(Locale::from_code("ZH"), Locale::ZhCN);
        assert_eq!(Locale::from_code("zh-CN"), Locale::ZhCN);
        assert_eq!(Locale::from_code(" zh_cn "), Locale::ZhCN);
        assert_eq!(Locale::from_code("ZH-HANS"), Locale::ZhCN);
        assert_eq!(Locale::from_code("unknown"), Locale::EnUS);
        assert_eq!(Locale::from_code(""), Locale::EnUS);
    }
}

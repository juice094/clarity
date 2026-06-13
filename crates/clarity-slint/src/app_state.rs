//! 阶段 1 的 Rust 后端状态。
//!
//! 当前按手绘图布局组织数据：左侧树形导航、中央 Bot/输入区、右侧抽屉。

use slint::{SharedString, VecModel};
use std::rc::Rc;

/// 树节点（Rust 侧真实结构）。
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// 节点唯一标识。
    pub id: String,
    /// 显示文本。
    pub label: String,
    /// 子节点。
    pub children: Vec<TreeNode>,
    /// 是否展开。
    pub expanded: bool,
}

impl TreeNode {
    /// 创建叶子节点。
    pub fn leaf(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            expanded: false,
        }
    }

    /// 创建可展开父节点。
    pub fn group(id: impl Into<String>, label: impl Into<String>, children: Vec<TreeNode>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children,
            expanded: true,
        }
    }

    /// 查找并切换指定 id 节点的展开状态。
    pub fn toggle(&mut self, id: &str) -> bool {
        if self.id == id && !self.children.is_empty() {
            self.expanded = !self.expanded;
            return true;
        }
        for child in &mut self.children {
            if child.toggle(id) {
                return true;
            }
        }
        false
    }
}

/// 应用状态。
pub struct AppState {
    core_actions: Vec<String>,
    external_links: Vec<String>,
    extra_features: Vec<String>,
    claw_devices: Vec<String>,
    workspace_tree: Vec<TreeNode>,
    bot_name: String,
    user_name: String,
    input_text: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self::mock()
    }
}

impl AppState {
    /// 创建模拟数据，用于阶段 1 布局验证。
    pub fn mock() -> Self {
        Self {
            core_actions: vec![
                "新建会话".to_string(),
                "技能".to_string(),
                "定时任务".to_string(),
            ],
            external_links: vec!["网页外链1".to_string(), "网页外链2".to_string()],
            extra_features: vec!["功能1".to_string(), "功能2".to_string()],
            claw_devices: vec!["设备1 实例".to_string(), "设备2 实例".to_string()],
            workspace_tree: vec![
                TreeNode::group(
                    "project-clarity",
                    "clarity",
                    vec![
                        TreeNode::leaf("session-a", "项目1会话1"),
                        TreeNode::leaf("session-b", "项目1会话2"),
                    ],
                ),
                TreeNode::group(
                    "project-syncthing",
                    "syncthing-rust",
                    vec![TreeNode::leaf("session-c", "项目2会话1")],
                ),
                TreeNode::group(
                    "free-sessions",
                    "对话",
                    vec![
                        TreeNode::leaf("session-d", "无项目会话1"),
                        TreeNode::leaf("session-e", "无项目会话2"),
                    ],
                ),
            ],
            bot_name: "代码助手".to_string(),
            user_name: "酒宿_juice".to_string(),
            input_text: String::new(),
        }
    }

    // --- 左侧导航数据 ---

    /// 核心入口列表。
    pub fn core_actions_model(&self) -> Rc<VecModel<SharedString>> {
        Rc::new(VecModel::from(
            self.core_actions
                .iter()
                .map(|s| SharedString::from(s.as_str()))
                .collect::<Vec<_>>(),
        ))
    }

    /// 外部链接列表。
    pub fn external_links_model(&self) -> Rc<VecModel<SharedString>> {
        Rc::new(VecModel::from(
            self.external_links
                .iter()
                .map(|s| SharedString::from(s.as_str()))
                .collect::<Vec<_>>(),
        ))
    }

    /// 扩展功能列表。
    pub fn extra_features_model(&self) -> Rc<VecModel<SharedString>> {
        Rc::new(VecModel::from(
            self.extra_features
                .iter()
                .map(|s| SharedString::from(s.as_str()))
                .collect::<Vec<_>>(),
        ))
    }

    /// Claw 设备列表。
    pub fn claw_devices_model(&self) -> Rc<VecModel<SharedString>> {
        Rc::new(VecModel::from(
            self.claw_devices
                .iter()
                .map(|s| SharedString::from(s.as_str()))
                .collect::<Vec<_>>(),
        ))
    }

    /// 返回拍平后的树模型供 Slint 渲染。
    pub fn tree_items_model(&self) -> Rc<VecModel<crate::ui::TreeItem>> {
        let mut items = Vec::new();
        Self::flatten(&self.workspace_tree, 0, &mut items);
        Rc::new(VecModel::from(items))
    }

    fn flatten(nodes: &[TreeNode], depth: usize, output: &mut Vec<crate::ui::TreeItem>) {
        for node in nodes {
            output.push(crate::ui::TreeItem {
                id: SharedString::from(node.id.as_str()),
                label: SharedString::from(node.label.as_str()),
                depth: depth as i32,
                is_expanded: node.expanded,
                has_children: !node.children.is_empty(),
            });
            if node.expanded && !node.children.is_empty() {
                Self::flatten(&node.children, depth + 1, output);
            }
        }
    }

    /// 切换指定 id 节点的展开状态。
    pub fn toggle_tree_item(&mut self, id: &str) {
        for node in &mut self.workspace_tree {
            if node.toggle(id) {
                break;
            }
        }
    }

    /// 当前用户显示名。
    pub fn user_name(&self) -> SharedString {
        SharedString::from(self.user_name.as_str())
    }

    // --- 中央聊天区数据 ---

    /// Bot 头部显示名。
    pub fn bot_name(&self) -> SharedString {
        SharedString::from(self.bot_name.as_str())
    }

    /// 输入框当前文本。
    pub fn input_text(&self) -> SharedString {
        SharedString::from(self.input_text.as_str())
    }

    /// 设置输入框文本。
    pub fn set_input_text(&mut self, text: impl Into<String>) {
        self.input_text = text.into();
    }
}

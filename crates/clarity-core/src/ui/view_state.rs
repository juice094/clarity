/// 主视图枚举 — GUI 和 TUI 共享同一套视图语义。
///
/// TUI 因屏幕限制一次只显示一个主视图；
/// GUI 允许侧边栏叠加，但主视图必须互斥。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppView {
    /// 主聊天视图（默认）
    Chat,
    /// 配置面板（属性检查器风格）
    Settings,
    /// 系统仪表盘
    Dashboard,
    /// 甘特图 / 时间线
    Gantt,
    /// 任务看板
    TaskBoard,
}

impl Default for AppView {
    fn default() -> Self {
        Self::Chat
    }
}

/// 侧边栏面板类型 — 仅在 GUI 中支持左右叠加，TUI 中通过模态切换实现。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidePanel {
    /// 左侧导航 / 角色列表
    Sidebar,
    /// 右侧文件/任务工作区
    Workspace,
    /// 团队协作面板
    Team,
    /// 任务详情
    Task,
}

/// 阻断式弹窗类型 — 最上层，接收独占输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalType {
    /// 操作审批
    Approval,
    /// 快照恢复
    Snapshot,
    /// OAuth 登录
    Login,
    /// 新建任务
    TaskCreate,
    /// 新建团队
    TeamCreate,
}

/// 统一的视图状态 — 两端共享的单一真相源。
///
/// 规则：
/// - `main` 切换时，`left` / `right` 保持打开状态（除非空间不足触发 responsive guard）
/// - `modal` 存在时，`main` / `left` / `right` 渲染但接收不到输入事件
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewState {
    pub main: AppView,
    pub left: Option<SidePanel>,
    pub right: Option<SidePanel>,
    pub modal: Option<ModalType>,
}

impl ViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 切换主视图，保持侧边栏状态。
    pub fn switch_main(&mut self, view: AppView) {
        self.main = view;
    }

    /// 打开或关闭左侧面板（互斥：一次只能有一个 left panel）。
    pub fn toggle_left(&mut self, panel: SidePanel) {
        self.left = if self.left == Some(panel) { None } else { Some(panel) };
    }

    /// 打开或关闭右侧面板（互斥：一次只能有一个 right panel）。
    pub fn toggle_right(&mut self, panel: SidePanel) {
        self.right = if self.right == Some(panel) { None } else { Some(panel) };
    }

    /// 打开模态弹窗。
    pub fn open_modal(&mut self, modal: ModalType) {
        self.modal = Some(modal);
    }

    /// 关闭当前模态弹窗。
    pub fn close_modal(&mut self) {
        self.modal = None;
    }

    /// 检查当前是否有任何面板会挤压主内容区。
    pub fn has_panels(&self) -> bool {
        self.left.is_some() || self.right.is_some()
    }
}

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

/// 入口类型 —— 窗口即认知边界
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryPoint {
    /// 生活/陪伴入口：永存且唯一
    Claw,
    /// 查询/问答入口：短且不唯一
    Window,
    /// 工程/开发入口：长且不唯一
    Cli,
}

impl std::fmt::Display for EntryPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryPoint::Claw => write!(f, "claw"),
            EntryPoint::Window => write!(f, "window"),
            EntryPoint::Cli => write!(f, "cli"),
        }
    }
}

/// 会话 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// 会话中的消息记录
#[derive(Debug, Clone)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// 会话信息
#[allow(dead_code)]
#[derive(Debug)]
pub struct Session {
    pub id: SessionId,
    pub entry: EntryPoint,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub message_count: u64,
    pub messages: Vec<SessionMessage>,
}

#[allow(dead_code)]
impl Session {
    pub fn new(id: SessionId, entry: EntryPoint) -> Self {
        let now = Utc::now();
        Self {
            id,
            entry,
            created_at: now,
            last_activity: now,
            message_count: 0,
            messages: Vec::new(),
        }
    }

    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }

    pub fn increment_message_count(&mut self) {
        self.message_count += 1;
        self.touch();
    }

    pub fn record_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.messages.push(SessionMessage {
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
        });
        self.increment_message_count();
    }

    pub fn get_messages(&self) -> &[SessionMessage] {
        &self.messages
    }
}

/// 按入口隔离的会话管理器
#[derive(Debug)]
pub struct SessionManager {
    claw: HashMap<SessionId, Session>,
    window: HashMap<SessionId, Session>,
    cli: HashMap<SessionId, Session>,
    total_requests: AtomicU64,
    started_at: DateTime<Utc>,
}

#[allow(dead_code)]
impl SessionManager {
    pub fn new() -> Self {
        Self {
            claw: HashMap::new(),
            window: HashMap::new(),
            cli: HashMap::new(),
            total_requests: AtomicU64::new(0),
            started_at: Utc::now(),
        }
    }

    fn namespace(&self, entry: EntryPoint) -> &HashMap<SessionId, Session> {
        match entry {
            EntryPoint::Claw => &self.claw,
            EntryPoint::Window => &self.window,
            EntryPoint::Cli => &self.cli,
        }
    }

    fn namespace_mut(&mut self, entry: EntryPoint) -> &mut HashMap<SessionId, Session> {
        match entry {
            EntryPoint::Claw => &mut self.claw,
            EntryPoint::Window => &mut self.window,
            EntryPoint::Cli => &mut self.cli,
        }
    }

    /// 创建新会话
    pub fn create_session(&mut self, id: SessionId, entry: EntryPoint) -> &mut Session {
        let session = Session::new(id.clone(), entry);
        self.namespace_mut(entry).insert(id.clone(), session);
        tracing::info!("Session created: {} ({})", id, entry);
        self.namespace_mut(entry).get_mut(&id).unwrap()
    }

    /// 销毁会话
    pub fn destroy_session(&mut self, id: &SessionId, entry: EntryPoint) {
        if self.namespace_mut(entry).remove(id).is_some() {
            tracing::info!("Session destroyed: {} ({})", id, entry);
        }
    }

    /// 获取会话
    pub fn get_session(&self, id: &SessionId, entry: EntryPoint) -> Option<&Session> {
        self.namespace(entry).get(id)
    }

    /// 获取可变会话
    pub fn get_session_mut(&mut self, id: &SessionId, entry: EntryPoint) -> Option<&mut Session> {
        self.namespace_mut(entry).get_mut(id)
    }

    /// 活跃会话数（全部入口）
    pub fn active_session_count(&self) -> usize {
        self.claw.len() + self.window.len() + self.cli.len()
    }

    /// 按入口的活跃会话数
    pub fn active_session_count_by(&self, entry: EntryPoint) -> usize {
        self.namespace(entry).len()
    }

    /// 记录请求
    pub fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::SeqCst);
    }

    /// 总请求数
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::SeqCst)
    }

    /// 运行时间（秒）
    pub fn uptime_seconds(&self) -> u64 {
        (Utc::now() - self.started_at).num_seconds() as u64
    }

    /// 清理过期会话
    pub fn cleanup_expired(&mut self, max_idle_minutes: i64) {
        let now = Utc::now();
        for entry in [EntryPoint::Claw, EntryPoint::Window, EntryPoint::Cli] {
            let expired: Vec<SessionId> = self
                .namespace(entry)
                .iter()
                .filter(|(_, s)| (now - s.last_activity).num_minutes() > max_idle_minutes)
                .map(|(id, _)| id.clone())
                .collect();

            for id in &expired {
                self.namespace_mut(entry).remove(id);
                tracing::info!("Session expired: {} ({})", id, entry);
            }
        }
    }

    /// 获取所有会话信息
    pub fn get_all_sessions(&self) -> Vec<&Session> {
        let mut all = Vec::new();
        all.extend(self.claw.values());
        all.extend(self.window.values());
        all.extend(self.cli.values());
        all
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_generation() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1.0, id2.0);
    }

    #[test]
    fn test_session_creation() {
        let mut manager = SessionManager::new();
        let id = SessionId::new();
        let session = manager.create_session(id.clone(), EntryPoint::Window);

        assert_eq!(session.id.0, id.0);
        assert_eq!(session.entry, EntryPoint::Window);
        assert_eq!(session.message_count, 0);
        assert!(session.messages.is_empty());
    }

    #[test]
    fn test_session_isolation() {
        let mut manager = SessionManager::new();
        let id = SessionId::new();

        manager.create_session(id.clone(), EntryPoint::Window);
        assert_eq!(manager.active_session_count_by(EntryPoint::Window), 1);
        assert_eq!(manager.active_session_count_by(EntryPoint::Cli), 0);

        // 同一个 ID 在不同入口可以共存
        manager.create_session(id.clone(), EntryPoint::Cli);
        assert_eq!(manager.active_session_count_by(EntryPoint::Window), 1);
        assert_eq!(manager.active_session_count_by(EntryPoint::Cli), 1);
    }

    #[test]
    fn test_session_message_tracking() {
        let mut manager = SessionManager::new();
        let id = SessionId::new();
        let session = manager.create_session(id.clone(), EntryPoint::Window);

        session.record_message("user", "Hello");
        assert_eq!(session.message_count, 1);
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, "user");
        assert_eq!(session.messages[0].content, "Hello");

        session.record_message("assistant", "Hi there!");
        assert_eq!(session.message_count, 2);
        assert_eq!(session.messages.len(), 2);
    }

    #[test]
    fn test_session_count() {
        let mut manager = SessionManager::new();
        assert_eq!(manager.active_session_count(), 0);

        let id1 = SessionId::new();
        manager.create_session(id1, EntryPoint::Window);
        assert_eq!(manager.active_session_count(), 1);
        assert_eq!(manager.active_session_count_by(EntryPoint::Window), 1);

        let id2 = SessionId::new();
        manager.create_session(id2, EntryPoint::Claw);
        assert_eq!(manager.active_session_count(), 2);
        assert_eq!(manager.active_session_count_by(EntryPoint::Claw), 1);
    }

    #[test]
    fn test_total_requests() {
        let manager = SessionManager::new();
        assert_eq!(manager.total_requests(), 0);

        manager.record_request();
        manager.record_request();
        assert_eq!(manager.total_requests(), 2);
    }
}

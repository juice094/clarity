use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

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

/// 会话信息
#[derive(Debug)]
pub struct Session {
    pub id: SessionId,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub message_count: u64,
}

impl Session {
    pub fn new(id: SessionId) -> Self {
        let now = Utc::now();
        Self {
            id,
            created_at: now,
            last_activity: now,
            message_count: 0,
        }
    }

    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }

    pub fn increment_message_count(&mut self) {
        self.message_count += 1;
        self.touch();
    }
}

/// 会话管理器
#[derive(Debug)]
pub struct SessionManager {
    sessions: HashMap<SessionId, Session>,
    total_requests: AtomicU64,
    started_at: DateTime<Utc>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            total_requests: AtomicU64::new(0),
            started_at: Utc::now(),
        }
    }

    /// 创建新会话
    pub fn create_session(&mut self, id: SessionId) -> &mut Session {
        let session = Session::new(id.clone());
        self.sessions.insert(id.clone(), session);
        tracing::info!("Session created: {}", id);
        self.sessions.get_mut(&id).unwrap()
    }

    /// 销毁会话
    pub fn destroy_session(&mut self, id: &SessionId) {
        if self.sessions.remove(id).is_some() {
            tracing::info!("Session destroyed: {}", id);
        }
    }

    /// 获取会话
    pub fn get_session(&self, id: &SessionId) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// 获取可变会话
    pub fn get_session_mut(&mut self, id: &SessionId) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    /// 活跃会话数
    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
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
        let expired: Vec<SessionId> = self
            .sessions
            .iter()
            .filter(|(_, s)| (now - s.last_activity).num_minutes() > max_idle_minutes)
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired {
            self.destroy_session(&id);
        }
    }

    /// 获取所有会话信息
    pub fn get_all_sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
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
        let session = manager.create_session(id.clone());

        assert_eq!(session.id.0, id.0);
        assert_eq!(session.message_count, 0);
    }

    #[test]
    fn test_session_count() {
        let mut manager = SessionManager::new();
        assert_eq!(manager.active_session_count(), 0);

        let id1 = SessionId::new();
        manager.create_session(id1);
        assert_eq!(manager.active_session_count(), 1);

        let id2 = SessionId::new();
        manager.create_session(id2.clone());
        assert_eq!(manager.active_session_count(), 2);

        manager.destroy_session(&id2);
        assert_eq!(manager.active_session_count(), 1);
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

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
    /// Generate a new random session identifier.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_generation() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1.0, id2.0);
    }
}

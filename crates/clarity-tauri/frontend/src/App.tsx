import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import TaskPanel from "./components/TaskPanel";
import ComputerUsePanel from "./components/ComputerUsePanel";
import SettingsPanel, { type GuiSettings } from "./components/SettingsPanel";
import FileBrowser from "./components/FileBrowser";
import DiffViewer, { type DiffHunk } from "./components/DiffViewer";
import LspPanel from "./components/LspPanel";
import Sidebar, {
  createNewSession,
  type Session,
  type Message,
} from "./components/Sidebar";
import "./App.css";

interface MessageData {
  role: string;
  content: string;
}

function DiffPanel({ isOpen, hunks, onClose }: { isOpen: boolean; hunks: DiffHunk[]; onClose: () => void }) {
  if (!isOpen) return null;
  return (
    <div className="diff-panel">
      <div className="diff-panel-header">
        <h2>Diff Preview</h2>
        <button className="diff-panel-close" onClick={onClose} aria-label="Close diff panel">
          ✕
        </button>
      </div>
      <div className="diff-panel-body">
        {hunks.length === 0 ? (
          <div className="diff-empty">No diff to display</div>
        ) : (
          <DiffViewer hunks={hunks} />
        )}
      </div>
    </div>
  );
}

interface SessionData {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  messages: MessageData[];
}

function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string>("");
  const [hasLoaded, setHasLoaded] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const activeSession = sessions.find((s) => s.id === activeSessionId);
  const [messages, setMessages] = useState<Message[]>(
    activeSession?.messages ?? []
  );
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [status, setStatus] = useState("unconfigured");
  const [version, setVersion] = useState("");
  const [taskPanelOpen, setTaskPanelOpen] = useState(false);
  const [computerPanelOpen, setComputerPanelOpen] = useState(false);
  const [settingsPanelOpen, setSettingsPanelOpen] = useState(false);
  const [fileBrowserOpen, setFileBrowserOpen] = useState(false);
  const [diffPanelOpen, setDiffPanelOpen] = useState(false);
  const [diffHunks, setDiffHunks] = useState<DiffHunk[]>([]);
  const [lspPanelOpen, setLspPanelOpen] = useState(false);
  const [theme, setTheme] = useState("dark");
  const [networkStatus, setNetworkStatus] = useState<"offline" | "restored" | "error" | null>(null);
  const [networkErrorMsg, setNetworkErrorMsg] = useState<string>("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const streamingRef = useRef(false);
  const taskIdRef = useRef<string | null>(null);

  useEffect(() => {
    invoke<string>("get_app_version").then(setVersion);
    refreshStatus();
    invoke<string | null>("get_prewarm_status").then((err) => {
      if (err) {
        setNetworkErrorMsg(err);
        setNetworkStatus("error");
        setTimeout(() => setNetworkStatus((s) => (s === "error" ? null : s)), 8000);
      }
    });
  }, []);

  // 加载持久化会话
  useEffect(() => {
    invoke<SessionData[]>("list_sessions")
      .then((data) => {
        if (data.length > 0) {
          const loaded = data.map((s) => ({
            id: s.id,
            title: s.title,
            created_at: s.created_at,
            updated_at: s.updated_at,
            messages: s.messages.map((m) => ({
              role: m.role as "user" | "agent",
              content: m.content,
            })),
          }));
          setSessions(loaded);
          setActiveSessionId(loaded[0].id);
          setMessages(loaded[0].messages);
        } else {
          const initial = createNewSession();
          setSessions([initial]);
          setActiveSessionId(initial.id);
          setMessages([]);
        }
        setHasLoaded(true);
      })
      .catch((e) => {
        console.error("Failed to load sessions:", e);
        const initial = createNewSession();
        setSessions([initial]);
        setActiveSessionId(initial.id);
        setMessages([]);
        setHasLoaded(true);
      });
  }, []);

  // 自动保存当前会话（debounce 500ms）
  useEffect(() => {
    if (!hasLoaded) return;
    const activeSession = sessions.find((s) => s.id === activeSessionId);
    if (!activeSession) return;

    const timeout = setTimeout(() => {
      invoke("save_session", {
        session: {
          id: activeSession.id,
          title: activeSession.title,
          created_at: activeSession.created_at,
          updated_at: activeSession.updated_at,
          messages: activeSession.messages,
        },
      }).catch((e) => console.error("Failed to save session:", e));
    }, 500);

    return () => clearTimeout(timeout);
  }, [sessions, activeSessionId, hasLoaded]);

  // 加载设置中的 theme
  useEffect(() => {
    invoke<GuiSettings>("get_settings").then((s) => {
      const t = s.theme;
      if (t === "auto") {
        const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
        setTheme(prefersDark ? "dark" : "light");
      } else {
        setTheme(t);
      }
    });
  }, []);

  // 监听系统主题变化（Auto 模式）
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => {
      invoke<GuiSettings>("get_settings").then((s) => {
        if (s.theme === "auto") {
          setTheme(e.matches ? "dark" : "light");
        }
      });
    };
    media.addEventListener("change", handler);
    return () => media.removeEventListener("change", handler);
  }, []);

  // 应用主题到 document
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isLoading]);

  // Sync messages back to active session whenever they change
  useEffect(() => {
    setSessions((prev) => {
      const session = prev.find((s) => s.id === activeSessionId);
      if (session && session.messages !== messages) {
        return prev.map((s) =>
          s.id === activeSessionId
            ? { ...s, messages, updated_at: Date.now() }
            : s
        );
      }
      return prev;
    });
  }, [messages, activeSessionId]);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    listen<string>("agent:chunk", (event) => {
      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (last && last.role === "agent") {
          const updated = [...prev];
          updated[updated.length - 1] = {
            ...last,
            content: last.content + event.payload,
          };
          return updated;
        }
        return prev;
      });
    }).then((u) => unlisteners.push(u));

    listen<string | null>("agent:done", () => {
      streamingRef.current = false;
      setIsLoading(false);
      refreshStatus();
      if (taskIdRef.current) {
        invoke("complete_task", { id: taskIdRef.current, status: "completed" }).catch(console.error);
        taskIdRef.current = null;
      }
    }).then((u) => unlisteners.push(u));

    listen<string>("agent:error", (event) => {
      streamingRef.current = false;
      setIsLoading(false);
      setMessages((prev) => [
        ...prev,
        { role: "agent", content: `Error: ${event.payload}` },
      ]);
      refreshStatus();
      if (taskIdRef.current) {
        invoke("complete_task", { id: taskIdRef.current, status: "failed" }).catch(console.error);
        taskIdRef.current = null;
      }
    }).then((u) => unlisteners.push(u));

    listen<{ fallback: boolean; reason: string }>("llm:fallback", (event) => {
      if (event.payload.fallback) {
        setNetworkStatus("offline");
      } else {
        setNetworkStatus("restored");
        // Auto-dismiss restored banner after 5s
        setTimeout(() => setNetworkStatus((s) => (s === "restored" ? null : s)), 5000);
      }
    }).then((u) => unlisteners.push(u));

    listen<{ message: string; context: string }>("llm:fallback_error", (event) => {
      setNetworkErrorMsg(event.payload.message);
      setNetworkStatus("error");
      setTimeout(() => setNetworkStatus((s) => (s === "error" ? null : s)), 8000);
    }).then((u) => unlisteners.push(u));

    listen<{ message: string }>("llm:config_error", (event) => {
      setNetworkErrorMsg(event.payload.message);
      setNetworkStatus("error");
      setTimeout(() => setNetworkStatus((s) => (s === "error" ? null : s)), 8000);
    }).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  async function refreshStatus() {
    const s = await invoke<string>("get_agent_status");
    setStatus(s);
  }

  function handleSelectSession(id: string) {
    if (streamingRef.current && id !== activeSessionId) {
      invoke("agent_interrupt");
      streamingRef.current = false;
      setIsLoading(false);
      if (taskIdRef.current) {
        invoke("complete_task", { id: taskIdRef.current, status: "failed" }).catch(console.error);
        taskIdRef.current = null;
      }
    }
    setActiveSessionId(id);
    const session = sessions.find((s) => s.id === id);
    if (session) {
      setMessages(session.messages);
    }
  }

  function handleNewSession() {
    if (streamingRef.current) {
      invoke("agent_interrupt");
      streamingRef.current = false;
      setIsLoading(false);
      if (taskIdRef.current) {
        invoke("complete_task", { id: taskIdRef.current, status: "failed" }).catch(console.error);
        taskIdRef.current = null;
      }
    }
    const newSession = createNewSession();
    setSessions((prev) => [newSession, ...prev]);
    setActiveSessionId(newSession.id);
    setMessages([]);
    invoke("save_session", {
      session: {
        id: newSession.id,
        title: newSession.title,
        created_at: newSession.created_at,
        updated_at: newSession.updated_at,
        messages: newSession.messages,
      },
    }).catch((e) => console.error("Failed to save new session:", e));
  }

  function handleDeleteSession(id: string) {
    if (streamingRef.current && id === activeSessionId) {
      invoke("agent_interrupt");
      streamingRef.current = false;
      setIsLoading(false);
      if (taskIdRef.current) {
        invoke("complete_task", { id: taskIdRef.current, status: "failed" }).catch(console.error);
        taskIdRef.current = null;
      }
    }
    const newSessions = sessions.filter((s) => s.id !== id);
    if (newSessions.length === 0) {
      const newSession = createNewSession();
      setSessions([newSession]);
      setActiveSessionId(newSession.id);
      setMessages([]);
      invoke("save_session", {
        session: {
          id: newSession.id,
          title: newSession.title,
          created_at: newSession.created_at,
          updated_at: newSession.updated_at,
          messages: newSession.messages,
        },
      }).catch((e) => console.error("Failed to save session:", e));
    } else {
      setSessions(newSessions);
      if (id === activeSessionId) {
        const next = newSessions[0];
        setActiveSessionId(next.id);
        setMessages(next.messages);
      }
    }
    invoke("delete_session", { id }).catch((e) =>
      console.error("Failed to delete session:", e)
    );
  }

  function handleRenameSession(id: string, newTitle: string) {
    setSessions((prev) =>
      prev.map((s) => (s.id === id ? { ...s, title: newTitle } : s))
    );
  }

  async function sendMessage() {
    if (!input.trim() || isLoading) return;
    const query = input.trim();
    setInput("");
    setMessages((prev) => [...prev, { role: "user", content: query }]);
    setIsLoading(true);
    streamingRef.current = true;
    await refreshStatus();

    // Create task record
    try {
      const taskId = await invoke<string>("create_task", { name: query.slice(0, 30) });
      taskIdRef.current = taskId;
    } catch (e) {
      console.error("Failed to create task:", e);
    }

    // Placeholder for the streaming response
    setMessages((prev) => [...prev, { role: "agent", content: "" }]);

    try {
      await invoke("agent_run_streaming", { query });
    } catch (e) {
      // Fallback: if the streaming command itself fails, show error
      if (streamingRef.current) {
        streamingRef.current = false;
        setIsLoading(false);
        setMessages((prev) => {
          const updated = [...prev];
          const last = updated[updated.length - 1];
          if (last && last.role === "agent" && last.content === "") {
            updated[updated.length - 1] = {
              ...last,
              content: `Error: ${e}`,
            };
          } else {
            updated.push({ role: "agent", content: `Error: ${e}` });
          }
          return updated;
        });
        if (taskIdRef.current) {
          invoke("complete_task", { id: taskIdRef.current, status: "failed" }).catch(console.error);
          taskIdRef.current = null;
        }
        await refreshStatus();
      }
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }

  return (
    <div className="app-layout">
      <Sidebar
        sessions={sessions}
        activeSessionId={activeSessionId}
        collapsed={sidebarCollapsed}
        onToggle={() => setSidebarCollapsed((prev) => !prev)}
        onSelect={handleSelectSession}
        onNew={handleNewSession}
        onDelete={handleDeleteSession}
        onRename={handleRenameSession}
      />
      <div className="chat-container">
        {networkStatus && (
          <div className={`network-banner ${networkStatus}`}>
            {networkStatus === "offline"
              ? "⚠️ Network unavailable — switched to local model"
              : networkStatus === "restored"
              ? "✅ Network restored — switched back to preferred provider"
              : `❌ ${networkErrorMsg}`}
          </div>
        )}
        <header className="chat-header">
          <div className="header-left">
            <button
              className="sidebar-toggle"
              onClick={() => setSidebarCollapsed((prev) => !prev)}
              title="Toggle sidebar"
              aria-label="Toggle sidebar"
            >
              ☰
            </button>
            <h1>Clarity</h1>
          </div>
          <div className="header-meta">
            <span className="version">v{version}</span>
            <span className={`status-badge ${status}`}>{status}</span>
            <button
              className="file-browser-toggle-btn"
              onClick={() => setFileBrowserOpen((prev) => !prev)}
              title="Browse files"
              aria-label="Browse files"
            >
              📂
            </button>
            <button
              className="task-toggle-btn"
              onClick={() => setTaskPanelOpen((prev) => !prev)}
              title="Toggle task panel"
              aria-label="Toggle task panel"
            >
              ⚡
            </button>
            <button
              className="computer-toggle-btn"
              onClick={() => setComputerPanelOpen((prev) => !prev)}
              title="Toggle computer use panel"
              aria-label="Toggle computer use panel"
            >
              🖥️
            </button>
            <button
              className="lsp-toggle-btn"
              onClick={() => setLspPanelOpen((prev) => !prev)}
              title="Toggle LSP panel"
              aria-label="Toggle LSP panel"
            >
              🔌 LSP
            </button>
            <button
              className="diff-toggle-btn"
              onClick={() => {
                if (!diffPanelOpen) {
                  invoke<DiffHunk[]>("compute_diff", {
                    oldText: "line1\nline2\nline3\n",
                    newText: "line1\nmodified\nline3\n",
                  }).then(setDiffHunks);
                }
                setDiffPanelOpen((prev) => !prev);
              }}
              title="Toggle diff panel"
              aria-label="Toggle diff panel"
            >
              📝
            </button>
            <button
              className="settings-toggle-btn"
              onClick={() => setSettingsPanelOpen((prev) => !prev)}
              title="Toggle settings"
              aria-label="Toggle settings"
            >
              ⚙️
            </button>
          </div>
        </header>

        <div className="main-content">
          <FileBrowser
            isOpen={fileBrowserOpen}
            onClose={() => setFileBrowserOpen(false)}
            onFileSelect={(path) => {
              setInput((prev) => prev + (prev ? " " : "") + `@${path}`);
              setFileBrowserOpen(false);
            }}
          />
          <div className="messages">
            {messages.length === 0 && (
              <div className="welcome">
                <h2>Welcome to Clarity</h2>
                <p>
                  Ask me anything. I can read files, run commands, and think
                  step by step.
                </p>
              </div>
            )}
            {messages.map((msg, i) => (
              <div key={i} className={`message ${msg.role}`}>
                <div className="message-bubble">{msg.content}</div>
              </div>
            ))}
            {isLoading &&
              messages[messages.length - 1]?.role !== "agent" && (
                <div className="message agent loading">
                  <div className="message-bubble">
                    <span className="dot-flashing" />
                  </div>
                </div>
              )}
            <div ref={messagesEndRef} />
          </div>
          <TaskPanel
            isOpen={taskPanelOpen}
            onClose={() => setTaskPanelOpen(false)}
          />
          <ComputerUsePanel
            isOpen={computerPanelOpen}
            onClose={() => setComputerPanelOpen(false)}
          />
          <LspPanel
            isOpen={lspPanelOpen}
            onClose={() => setLspPanelOpen(false)}
          />
          <DiffPanel isOpen={diffPanelOpen} hunks={diffHunks} onClose={() => setDiffPanelOpen(false)} />
          <SettingsPanel
            isOpen={settingsPanelOpen}
            onClose={() => setSettingsPanelOpen(false)}
          />
        </div>

        <div className="input-area">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a message..."
            rows={1}
            disabled={isLoading}
          />
          <button onClick={sendMessage} disabled={isLoading || !input.trim()}>
            Send
          </button>
        </div>
      </div>
    </div>
  );
}

export default App;

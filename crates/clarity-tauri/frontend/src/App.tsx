import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import TaskPanel from "./components/TaskPanel";
import Sidebar, {
  createNewSession,
  type Session,
  type Message,
} from "./components/Sidebar";
import "./App.css";

const initialSession = createNewSession();

function App() {
  const [sessions, setSessions] = useState<Session[]>([initialSession]);
  const [activeSessionId, setActiveSessionId] = useState<string>(
    initialSession.id
  );
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
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const streamingRef = useRef(false);

  useEffect(() => {
    invoke<string>("get_app_version").then(setVersion);
    refreshStatus();
  }, []);

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
    }).then((u) => unlisteners.push(u));

    listen<string>("agent:error", (event) => {
      streamingRef.current = false;
      setIsLoading(false);
      setMessages((prev) => [
        ...prev,
        { role: "agent", content: `Error: ${event.payload}` },
      ]);
      refreshStatus();
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
    setActiveSessionId(id);
    const session = sessions.find((s) => s.id === id);
    if (session) {
      setMessages(session.messages);
    }
  }

  function handleNewSession() {
    const newSession = createNewSession();
    setSessions((prev) => [newSession, ...prev]);
    setActiveSessionId(newSession.id);
    setMessages([]);
  }

  function handleDeleteSession(id: string) {
    const newSessions = sessions.filter((s) => s.id !== id);
    if (newSessions.length === 0) {
      const newSession = createNewSession();
      setSessions([newSession]);
      setActiveSessionId(newSession.id);
      setMessages([]);
    } else {
      setSessions(newSessions);
      if (id === activeSessionId) {
        const next = newSessions[0];
        setActiveSessionId(next.id);
        setMessages(next.messages);
      }
    }
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
              className="task-toggle-btn"
              onClick={() => setTaskPanelOpen((prev) => !prev)}
              title="Toggle task panel"
              aria-label="Toggle task panel"
            >
              ⚡
            </button>
          </div>
        </header>

        <div className="main-content">
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

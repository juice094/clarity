import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import "./App.css";

interface Message {
  role: "user" | "agent";
  content: string;
}

function App() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [status, setStatus] = useState("unconfigured");
  const [version, setVersion] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const streamingRef = useRef(false);

  useEffect(() => {
    invoke<string>("get_app_version").then(setVersion);
    refreshStatus();
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isLoading]);

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
    <div className="chat-container">
      <header className="chat-header">
        <h1>Clarity</h1>
        <div className="header-meta">
          <span className="version">v{version}</span>
          <span className={`status-badge ${status}`}>{status}</span>
        </div>
      </header>

      <div className="messages">
        {messages.length === 0 && (
          <div className="welcome">
            <h2>Welcome to Clarity</h2>
            <p>
              Ask me anything. I can read files, run commands, and think step
              by step.
            </p>
          </div>
        )}
        {messages.map((msg, i) => (
          <div key={i} className={`message ${msg.role}`}>
            <div className="message-bubble">{msg.content}</div>
          </div>
        ))}
        {isLoading && messages[messages.length - 1]?.role !== "agent" && (
          <div className="message agent loading">
            <div className="message-bubble">
              <span className="dot-flashing" />
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
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
  );
}

export default App;

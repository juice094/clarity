import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface LspPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

interface LspServerInfo {
  id: string;
  server_path: string;
  root_path: string;
  status: string;
}

const INITIALIZE_TEMPLATE = JSON.stringify(
  {
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      processId: null,
      rootUri: "file:///path/to/project",
      capabilities: {},
    },
  },
  null,
  2
);

function LspPanel({ isOpen, onClose }: LspPanelProps) {
  const [servers, setServers] = useState<LspServerInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const [serverPath, setServerPath] = useState("rust-analyzer");
  const [argsText, setArgsText] = useState("");
  const [rootPath, setRootPath] = useState("");

  const [selectedProcessId, setSelectedProcessId] = useState("");
  const [messageText, setMessageText] = useState(INITIALIZE_TEMPLATE);
  const [responseText, setResponseText] = useState("");

  const fetchServers = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const list = await invoke<LspServerInfo[]>("lsp_list");
      setServers(list);
    } catch (e) {
      const msg = String(e);
      setError(`Failed to list servers: ${msg}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      fetchServers();
    }
  }, [isOpen, fetchServers]);

  async function handleStart() {
    if (!serverPath.trim() || !rootPath.trim()) {
      setError("Server Path and Root Path are required");
      return;
    }
    setLoading(true);
    setError("");
    try {
      const args = argsText
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
      const id = await invoke<string>("lsp_start", {
        server_path: serverPath.trim(),
        args,
        root_path: rootPath.trim(),
      });
      setSelectedProcessId(id);
      await fetchServers();
    } catch (e) {
      const msg = String(e);
      setError(`Start failed: ${msg}`);
    } finally {
      setLoading(false);
    }
  }

  async function handleStop(processId: string) {
    setLoading(true);
    setError("");
    try {
      await invoke("lsp_stop", { process_id: processId });
      await fetchServers();
    } catch (e) {
      const msg = String(e);
      setError(`Stop failed: ${msg}`);
    } finally {
      setLoading(false);
    }
  }

  async function handleSend() {
    if (!selectedProcessId.trim() || !messageText.trim()) {
      setError("Process ID and message are required");
      return;
    }
    setLoading(true);
    setError("");
    try {
      await invoke("lsp_send", {
        process_id: selectedProcessId.trim(),
        message: messageText.trim(),
      });
      setResponseText((prev) => `${prev}\n[SENT to ${selectedProcessId}]`.trim());
    } catch (e) {
      const msg = String(e);
      setError(`Send failed: ${msg}`);
    } finally {
      setLoading(false);
    }
  }

  async function handleReceive() {
    if (!selectedProcessId.trim()) {
      setError("Process ID is required");
      return;
    }
    setLoading(true);
    setError("");
    try {
      const msg = await invoke<string | null>("lsp_recv", {
        process_id: selectedProcessId.trim(),
      });
      if (msg === null) {
        setResponseText((prev) =>
          `${prev ? prev + "\n" : ""}[RECV from ${selectedProcessId}]: (no message)`.trim()
        );
      } else {
        setResponseText((prev) =>
          `${prev ? prev + "\n" : ""}[RECV from ${selectedProcessId}]:\n${msg}`.trim()
        );
      }
    } catch (e) {
      const msg = String(e);
      setError(`Receive failed: ${msg}`);
    } finally {
      setLoading(false);
    }
  }

  if (!isOpen) return null;

  return (
    <div className="lsp-panel">
      <div className="lsp-panel-header">
        <h2>LSP Servers</h2>
        <button className="lsp-panel-close" onClick={onClose} aria-label="Close">
          ✕
        </button>
      </div>

      <div className="lsp-panel-body">
        {error && <div className="lsp-error">{error}</div>}

        {/* Server List */}
        <div className="lsp-section">
          <div className="lsp-section-header">
            <h3>Running Servers</h3>
            <button
              className="lsp-btn lsp-btn-secondary"
              onClick={fetchServers}
              disabled={loading}
              title="Refresh list"
            >
              🔄 Refresh
            </button>
          </div>
          <div className="lsp-server-list">
            {servers.length === 0 && (
              <div className="lsp-empty">No LSP servers running</div>
            )}
            {servers.map((s) => (
              <div key={s.id} className="lsp-server-item">
                <div className="lsp-server-info">
                  <span className="lsp-server-id">{s.id}</span>
                  <span className="lsp-server-path">{s.server_path}</span>
                  <span className="lsp-server-root">{s.root_path}</span>
                </div>
                <div className="lsp-server-actions">
                  <span
                    className={`lsp-status-badge ${s.status}`}
                    title={s.status}
                  >
                    {s.status}
                  </span>
                  <button
                    className="lsp-btn lsp-btn-danger"
                    onClick={() => handleStop(s.id)}
                    disabled={loading || s.status !== "running"}
                  >
                    Stop
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* New Server Form */}
        <div className="lsp-section">
          <h3>Start New Server</h3>
          <div className="lsp-form">
            <label className="lsp-label">
              Server Path
              <input
                type="text"
                className="lsp-input"
                value={serverPath}
                onChange={(e) => setServerPath(e.target.value)}
                placeholder="e.g. rust-analyzer"
              />
            </label>
            <label className="lsp-label">
              Args (comma-separated)
              <input
                type="text"
                className="lsp-input"
                value={argsText}
                onChange={(e) => setArgsText(e.target.value)}
                placeholder="e.g. --log-file,ra.log"
              />
            </label>
            <label className="lsp-label">
              Root Path
              <input
                type="text"
                className="lsp-input"
                value={rootPath}
                onChange={(e) => setRootPath(e.target.value)}
                placeholder="Project root directory"
              />
            </label>
            <button
              className="lsp-btn lsp-btn-primary"
              onClick={handleStart}
              disabled={loading}
            >
              Start
            </button>
          </div>
        </div>

        {/* Message Debug */}
        <div className="lsp-section">
          <h3>Message Debug</h3>
          <div className="lsp-form">
            <label className="lsp-label">
              Process ID
              <input
                type="text"
                className="lsp-input"
                value={selectedProcessId}
                onChange={(e) => setSelectedProcessId(e.target.value)}
                placeholder="e.g. lsp-1"
              />
            </label>
            <label className="lsp-label">
              Message (JSON)
              <textarea
                className="lsp-textarea"
                value={messageText}
                onChange={(e) => setMessageText(e.target.value)}
                rows={8}
              />
            </label>
            <div className="lsp-btn-row">
              <button
                className="lsp-btn lsp-btn-primary"
                onClick={handleSend}
                disabled={loading}
              >
                Send
              </button>
              <button
                className="lsp-btn lsp-btn-secondary"
                onClick={handleReceive}
                disabled={loading}
              >
                Receive
              </button>
            </div>
          </div>
          {responseText && (
            <div className="lsp-message-box">
              <h4>Response</h4>
              <pre className="lsp-message-pre">{responseText}</pre>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default LspPanel;

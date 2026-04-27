import { useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";

interface ComputerUsePanelProps {
  isOpen: boolean;
  onClose: () => void;
}

interface OperationLog {
  id: number;
  time: string;
  action: string;
  params: string;
  status: "success" | "error";
  error?: string;
}

function ComputerUsePanel({ isOpen, onClose }: ComputerUsePanelProps) {
  const [screenshot, setScreenshot] = useState<string>("");
  const [bridgeReady, setBridgeReady] = useState<boolean | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [logs, setLogs] = useState<OperationLog[]>([]);
  const [clickX, setClickX] = useState("");
  const [clickY, setClickY] = useState("");
  const [typeText, setTypeText] = useState("");
  const [scrollX, setScrollX] = useState("");
  const [scrollY, setScrollY] = useState("");
  const [scrollAmount, setScrollAmount] = useState("");
  const [imageCoords, setImageCoords] = useState<{ x: number; y: number } | null>(null);
  const logIdRef = useRef(0);

  const addLog = useCallback(
    (action: string, params: string, status: "success" | "error", errorMsg?: string) => {
      const id = ++logIdRef.current;
      const time = new Date().toLocaleTimeString();
      setLogs((prev) => [{ id, time, action, params, status, error: errorMsg }, ...prev]);
    },
    []
  );

  async function handleCheckBridge() {
    setLoading(true);
    setError("");
    try {
      const ok = await invoke<boolean>("computer_check_bridge");
      setBridgeReady(ok);
      if (!ok) {
        setError("Python bridge not available. Ensure python3/python and computer_bridge.py are accessible.");
      }
    } catch (e) {
      setBridgeReady(false);
      setError(`Bridge check failed: ${e}`);
    } finally {
      setLoading(false);
    }
  }

  async function handleScreenshot() {
    setLoading(true);
    setError("");
    try {
      const data = await invoke<string>("computer_screenshot");
      setScreenshot(data);
      setImageCoords(null);
      addLog("screenshot", "", "success");
    } catch (e) {
      const msg = String(e);
      setError(`Screenshot failed: ${msg}`);
      addLog("screenshot", "", "error", msg);
    } finally {
      setLoading(false);
    }
  }

  async function handleClick(x: number, y: number) {
    setLoading(true);
    setError("");
    try {
      await invoke("computer_click", { x, y });
      addLog("click", `x=${x}, y=${y}`, "success");
    } catch (e) {
      const msg = String(e);
      setError(`Click failed: ${msg}`);
      addLog("click", `x=${x}, y=${y}`, "error", msg);
    } finally {
      setLoading(false);
    }
  }

  async function handleType() {
    if (!typeText.trim()) return;
    setLoading(true);
    setError("");
    try {
      await invoke("computer_type", { text: typeText });
      addLog("type", `text=${typeText.slice(0, 40)}${typeText.length > 40 ? "..." : ""}`, "success");
      setTypeText("");
    } catch (e) {
      const msg = String(e);
      setError(`Type failed: ${msg}`);
      addLog("type", `text=${typeText.slice(0, 40)}`, "error", msg);
    } finally {
      setLoading(false);
    }
  }

  async function handleScroll() {
    const x = parseInt(scrollX, 10);
    const y = parseInt(scrollY, 10);
    const amount = parseInt(scrollAmount, 10);
    if (Number.isNaN(x) || Number.isNaN(y) || Number.isNaN(amount)) {
      setError("Invalid scroll coordinates or amount");
      return;
    }
    setLoading(true);
    setError("");
    try {
      await invoke("computer_scroll", { x, y, amount });
      addLog("scroll", `x=${x}, y=${y}, amount=${amount}`, "success");
    } catch (e) {
      const msg = String(e);
      setError(`Scroll failed: ${msg}`);
      addLog("scroll", `x=${x}, y=${y}, amount=${amount}`, "error", msg);
    } finally {
      setLoading(false);
    }
  }

  function handleImageClick(e: React.MouseEvent<HTMLImageElement>) {
    const x = e.nativeEvent.offsetX;
    const y = e.nativeEvent.offsetY;
    setImageCoords({ x, y });
    setClickX(String(x));
    setClickY(String(y));
  }

  function handleClickFromInputs() {
    const x = parseInt(clickX, 10);
    const y = parseInt(clickY, 10);
    if (Number.isNaN(x) || Number.isNaN(y)) {
      setError("Invalid click coordinates");
      return;
    }
    handleClick(x, y);
  }

  if (!isOpen) return null;

  return (
    <div className="computer-panel">
      <div className="computer-panel-header">
        <h2>Computer Use</h2>
        <button className="computer-panel-close" onClick={onClose} aria-label="Close">
          <X size={16} />
        </button>
      </div>

      <div className="computer-panel-body">
        {/* Bridge status */}
        {bridgeReady === false && (
          <div className="computer-error-banner">
            Python bridge unavailable. Click "Check Env" to verify.
          </div>
        )}
        {error && <div className="computer-error">{error}</div>}

        {/* Toolbar */}
        <div className="computer-toolbar">
          <button className="computer-btn" onClick={handleScreenshot} disabled={loading}>
            📷 Screenshot
          </button>
          <button className="computer-btn computer-btn-secondary" onClick={handleCheckBridge} disabled={loading}>
            🔍 Check Env
          </button>
        </div>

        {/* Screenshot display */}
        {screenshot && (
          <div className="computer-screenshot-wrapper">
            <img
              src={`data:image/png;base64,${screenshot}`}
              alt="Screenshot"
              className="computer-screenshot"
              onClick={handleImageClick}
              draggable={false}
            />
            {imageCoords && (
              <div className="computer-coords">
                Clicked: ({imageCoords.x}, {imageCoords.y})
                <button
                  className="computer-mini-btn"
                  onClick={() => handleClick(imageCoords.x, imageCoords.y)}
                  disabled={loading}
                >
                  Click here
                </button>
              </div>
            )}
          </div>
        )}

        {/* Actions */}
        <div className="computer-actions">
          <div className="computer-action-group">
            <h3>Click</h3>
            <div className="computer-input-row">
              <input
                type="number"
                placeholder="X"
                value={clickX}
                onChange={(e) => setClickX(e.target.value)}
                className="computer-input"
              />
              <input
                type="number"
                placeholder="Y"
                value={clickY}
                onChange={(e) => setClickY(e.target.value)}
                className="computer-input"
              />
              <button className="computer-btn" onClick={handleClickFromInputs} disabled={loading}>
                Click
              </button>
            </div>
          </div>

          <div className="computer-action-group">
            <h3>Type</h3>
            <div className="computer-input-row">
              <input
                type="text"
                placeholder="Text to type..."
                value={typeText}
                onChange={(e) => setTypeText(e.target.value)}
                className="computer-input computer-input-wide"
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleType();
                }}
              />
              <button className="computer-btn" onClick={handleType} disabled={loading}>
                Type
              </button>
            </div>
          </div>

          <div className="computer-action-group">
            <h3>Scroll</h3>
            <div className="computer-input-row">
              <input
                type="number"
                placeholder="X"
                value={scrollX}
                onChange={(e) => setScrollX(e.target.value)}
                className="computer-input"
              />
              <input
                type="number"
                placeholder="Y"
                value={scrollY}
                onChange={(e) => setScrollY(e.target.value)}
                className="computer-input"
              />
              <input
                type="number"
                placeholder="Amount"
                value={scrollAmount}
                onChange={(e) => setScrollAmount(e.target.value)}
                className="computer-input"
              />
              <button className="computer-btn" onClick={handleScroll} disabled={loading}>
                Scroll
              </button>
            </div>
          </div>
        </div>

        {/* Operation log */}
        <div className="computer-log">
          <h3>Operation Log</h3>
          <div className="computer-log-list">
            {logs.length === 0 && <div className="computer-log-empty">No operations yet</div>}
            {logs.map((log) => (
              <div key={log.id} className={`computer-log-item ${log.status}`}>
                <span className="computer-log-time">{log.time}</span>
                <span className="computer-log-action">{log.action}</span>
                <span className="computer-log-params">{log.params}</span>
                {log.status === "error" && log.error && (
                  <span className="computer-log-error">{log.error}</span>
                )}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export default ComputerUsePanel;

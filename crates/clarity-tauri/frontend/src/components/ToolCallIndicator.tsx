import { useState } from "react";
import { Loader2, Wrench, ChevronDown, ChevronUp, CheckCircle2 } from "lucide-react";

export interface ToolCallInfo {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  status: "running" | "done";
  result?: string;
}

interface ToolCallIndicatorProps {
  toolCalls: ToolCallInfo[];
}

function ToolCallCard({ call }: { call: ToolCallInfo }) {
  const [expanded, setExpanded] = useState(false);
  const isDone = call.status === "done";

  // Summarize arguments — show first key or count
  const argKeys = Object.keys(call.arguments);
  const argSummary =
    argKeys.length === 0
      ? "no args"
      : argKeys.length === 1
        ? `${argKeys[0]}=${String(call.arguments[argKeys[0]]).slice(0, 20)}`
        : `${argKeys.length} arguments`;

  return (
    <div className={`tool-call-card ${isDone ? "done" : ""}`}>
      <div className="tool-call-header" onClick={() => setExpanded((p) => !p)}>
        <div className="tool-call-icon">
          {isDone ? <CheckCircle2 size={14} /> : <Loader2 size={14} className="spin" />}
        </div>
        <div className="tool-call-meta">
          <span className="tool-call-name">{call.name}</span>
          <span className="tool-call-args">{argSummary}</span>
        </div>
        <button className="tool-call-toggle" aria-label={expanded ? "Collapse" : "Expand"}>
          {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
        </button>
      </div>
      {expanded && (
        <div className="tool-call-body">
          <div className="tool-call-section">
            <h4>Arguments</h4>
            <pre className="tool-call-json">
              {JSON.stringify(call.arguments, null, 2)}
            </pre>
          </div>
          {isDone && call.result !== undefined && (
            <div className="tool-call-section">
              <h4>Result</h4>
              <pre className="tool-call-json">
                {call.result.slice(0, 500)}
                {call.result.length > 500 ? "\n… truncated" : ""}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function ToolCallIndicator({ toolCalls }: ToolCallIndicatorProps) {
  if (toolCalls.length === 0) return null;

  return (
    <div className="tool-call-indicator">
      <div className="tool-call-label">
        <Wrench size={12} />
        <span>Tool calls</span>
      </div>
      <div className="tool-call-list">
        {toolCalls.map((call) => (
          <ToolCallCard key={call.id} call={call} />
        ))}
      </div>
    </div>
  );
}

export default ToolCallIndicator;

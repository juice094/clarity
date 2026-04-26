import { useState, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, ChevronLeft, Plus, Pencil, X } from "lucide-react";

export interface Message {
  role: "user" | "agent";
  content: string;
}

export interface Session {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  messages: Message[];
}

export interface ToolItem {
  id: string;
  label: string;
  icon: React.ReactNode;
  onClick: () => void;
}

interface SidebarProps {
  sessions: Session[];
  activeSessionId: string;
  collapsed: boolean;
  onToggle: () => void;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  onRename: (id: string, newTitle: string) => void;
  tools?: ToolItem[];
  activeTool?: string;
}

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

export function createNewSession(): Session {
  return {
    id: generateId(),
    title: "New Chat",
    created_at: Date.now(),
    updated_at: Date.now(),
    messages: [],
  };
}

function useFormatRelativeTime() {
  const { t } = useTranslation();
  return (timestamp: number): string => {
    const diff = Math.floor((Date.now() - timestamp) / 1000);
    if (diff < 60) return t("sidebar.justNow");
    if (diff < 3600) return t("sidebar.minutesAgo", { count: Math.floor(diff / 60) });
    if (diff < 86400) return t("sidebar.hoursAgo", { count: Math.floor(diff / 3600) });
    return t("sidebar.daysAgo", { count: Math.floor(diff / 86400) });
  };
}

export default function Sidebar({
  sessions,
  activeSessionId,
  collapsed,
  onToggle,
  onSelect,
  onNew,
  onDelete,
  onRename,
  tools = [],
  activeTool,
}: SidebarProps) {
  const { t } = useTranslation();
  const formatRelativeTime = useFormatRelativeTime();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const sortedSessions = [...sessions].sort((a, b) => b.updated_at - a.updated_at);

  const startRename = useCallback((session: Session) => {
    setEditingId(session.id);
    setEditValue(session.title);
    setTimeout(() => inputRef.current?.focus(), 0);
  }, []);

  const saveRename = useCallback(() => {
    if (editingId && editValue.trim()) {
      onRename(editingId, editValue.trim());
    }
    setEditingId(null);
    setEditValue("");
  }, [editingId, editValue, onRename]);

  const cancelRename = useCallback(() => {
    setEditingId(null);
    setEditValue("");
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        saveRename();
      } else if (e.key === "Escape") {
        cancelRename();
      }
    },
    [saveRename, cancelRename]
  );

  return (
    <aside className={`sidebar ${collapsed ? "collapsed" : ""}`}>
      <div className="sidebar-header">
        <button
          className="sidebar-toggle-btn"
          onClick={onToggle}
          title={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
        >
          {collapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
        </button>
        {!collapsed && (
          <>
            <span className="sidebar-title">Clarity</span>
            <button className="sidebar-new-btn" onClick={onNew} title={t("sidebar.newSession")}>
              <Plus size={16} />
            </button>
          </>
        )}
      </div>

      {!collapsed && tools.length > 0 && (
        <div className="tools-section">
          <div className="tools-header">Tools</div>
          {tools.map((tool) => (
            <button
              key={tool.id}
              className={`tool-item ${activeTool === tool.id ? "active" : ""}`}
              onClick={tool.onClick}
              title={tool.label}
            >
              {tool.icon}
              <span>{tool.label}</span>
            </button>
          ))}
        </div>
      )}

      {!collapsed && (
        <div className="session-list">
          {sortedSessions.map((session) => {
            const isActive = session.id === activeSessionId;
            const isEditing = session.id === editingId;

            return (
              <div
                key={session.id}
                className={`session-item ${isActive ? "active" : ""}`}
                onClick={() => onSelect(session.id)}
              >
                <div className="session-info">
                  {isEditing ? (
                    <input
                      ref={inputRef}
                      className="session-rename-input"
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                      onKeyDown={handleKeyDown}
                      onBlur={saveRename}
                      onClick={(e) => e.stopPropagation()}
                    />
                  ) : (
                    <>
                      <div className="session-title">{session.title}</div>
                      <div className="session-time">
                        {formatRelativeTime(session.updated_at)}
                      </div>
                    </>
                  )}
                </div>

                {!isEditing && (
                  <div className="session-actions">
                    <button
                      className="session-action-btn"
                      onClick={(e) => {
                        e.stopPropagation();
                        startRename(session);
                      }}
                      title={t("sidebar.rename")}
                    >
                      <Pencil size={14} />
                    </button>
                    <button
                      className="session-action-btn danger"
                      onClick={(e) => {
                        e.stopPropagation();
                        onDelete(session.id);
                      }}
                      title={t("sidebar.delete")}
                    >
                      <X size={14} />
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </aside>
  );
}

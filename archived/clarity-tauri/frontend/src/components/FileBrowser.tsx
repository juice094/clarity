import { useState, useEffect } from "react";
import { FolderOpen, Folder, X, FileText } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

interface FileTreeNode {
  name: string;
  type: "directory" | "file";
  path: string;
  size?: number;
  children?: FileTreeNode[];
}

interface FileBrowserProps {
  isOpen: boolean;
  onClose: () => void;
  onFileSelect: (path: string) => void;
}

export default function FileBrowser({
  isOpen,
  onClose,
  onFileSelect,
}: FileBrowserProps) {
  const [tree, setTree] = useState<FileTreeNode | null>(null);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (isOpen && tree === null) {
      setLoading(true);
      invoke<FileTreeNode>("get_file_tree", { path: null })
        .then((data) => {
          setTree(data);
        })
        .catch((err) => {
          console.error("Failed to load file tree:", err);
          setError("Failed to load file tree");
        })
        .finally(() => {
          setLoading(false);
        });
    }
  }, [isOpen, tree]);

  function toggleDir(path: string) {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }

  function renderNode(node: FileTreeNode, depth = 0): JSX.Element {
    const paddingLeft = depth * 16;

    if (node.type === "directory") {
      const isExpanded = expandedDirs.has(node.path);
      return (
        <div key={node.path}>
          <div
            className="file-tree-node file-tree-dir"
            style={{ paddingLeft: `${paddingLeft + 16}px` }}
            onClick={() => toggleDir(node.path)}
          >
            <span>{isExpanded ? <FolderOpen size={14} /> : <Folder size={14} />}</span>
            <span>{node.name}</span>
          </div>
          {isExpanded && node.children && (
            <div>
              {node.children.map((child) => renderNode(child, depth + 1))}
            </div>
          )}
        </div>
      );
    }

    return (
      <div
        key={node.path}
        className="file-tree-node file-tree-file"
        style={{ paddingLeft: `${paddingLeft + 16}px` }}
        onClick={() => onFileSelect(node.path)}
      >
        <span><FileText size={14} /></span>
        <span>{node.name}</span>
      </div>
    );
  }

  if (!isOpen) return null;

  return (
    <div className="file-browser-panel">
      <div className="file-browser-header">
        <h2>Files</h2>
        <button
          className="file-browser-close"
          onClick={onClose}
          title="Close"
          aria-label="Close file browser"
        >
          <X size={16} />
        </button>
      </div>
      <div className="file-browser-tree">
        {loading && <div className="file-tree-node">Loading...</div>}
        {!loading && tree === null && (
          <div className="file-tree-node">No files</div>
        )}
        {!loading && tree && renderNode(tree)}
      </div>
    </div>
  );
}

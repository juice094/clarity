/**
 * Clarity Files Module - File tree, direct file API
 * Fixed: tree-children no longer inside flex tree-item
 */

import * as api from './api.js';
import * as editor from './editor.js';

// ==================== DOM Refs ====================

const fileTreeEl = document.getElementById('file-tree');
const workspaceInput = document.getElementById('workspace-path');
const workspaceBtn = document.getElementById('workspace-btn');

let workspacePath = '.';

// ==================== Render Tree (fixed DOM structure) ====================

function renderTreeNode(node, container) {
    const isDir = node.type === 'directory';
    const hasChildren = isDir && node.children && node.children.length > 0;

    // Create the node wrapper (block-level, children are siblings)
    const nodeWrapper = document.createElement('div');
    nodeWrapper.className = 'tree-node';

    // Header row: toggle + icon + name
    const header = document.createElement('div');
    header.className = 'tree-item';
    header.dataset.path = node.path || '';
    header.title = node.path || node.name;

    const toggle = document.createElement('span');
    toggle.className = hasChildren ? 'tree-toggle' : 'tree-toggle hidden';
    toggle.textContent = '▶';

    const icon = document.createElement('span');
    icon.className = 'tree-icon';
    icon.textContent = isDir ? '📁' : getFileIcon(node.name);

    const label = document.createElement('span');
    label.className = 'tree-label';
    label.textContent = node.name;

    header.appendChild(toggle);
    header.appendChild(icon);
    header.appendChild(label);
    nodeWrapper.appendChild(header);

    // Children container (sibling of header, NOT child of flex header)
    let childrenContainer = null;
    if (isDir && hasChildren) {
        childrenContainer = document.createElement('div');
        childrenContainer.className = 'tree-children collapsed';
        for (const child of node.children) {
            renderTreeNode(child, childrenContainer);
        }
        nodeWrapper.appendChild(childrenContainer);

        // Toggle expand/collapse
        toggle.addEventListener('click', (e) => {
            e.stopPropagation();
            toggle.classList.toggle('expanded');
            childrenContainer.classList.toggle('collapsed');
        });

        // Click header to expand/collapse folder
        header.addEventListener('click', (e) => {
            if (e.target === toggle) return;
            toggle.click();
        });
    }

    // File click: open in editor
    if (!isDir) {
        header.addEventListener('click', () => {
            const path = header.dataset.path;
            if (path) {
                editor.openFile(path);
                document.querySelectorAll('.tree-item').forEach(i => i.classList.remove('active'));
                header.classList.add('active');
            }
        });
    }

    container.appendChild(nodeWrapper);
}

function getFileIcon(name) {
    const ext = name.split('.').pop()?.toLowerCase() || '';
    const icons = {
        rs: '🦀', js: '📜', ts: '📘', jsx: '⚛️', tsx: '⚛️',
        html: '🌐', css: '🎨', json: '📋', py: '🐍', go: '🐹',
        md: '📝', toml: '⚙️', yaml: '⚙️', sql: '🗄️',
        java: '☕', cpp: '⚙️', c: '⚙️', h: '⚙️',
        sh: '💻', ps1: '💻', txt: '📄',
    };
    return icons[ext] || '📄';
}

function escapeHtml(text) {
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ==================== Load Tree ====================

export function setWorkspace(path) {
    workspacePath = path || '.';
    if (workspaceInput) workspaceInput.value = workspacePath;
    loadTree();
}

async function loadTree() {
    fileTreeEl.innerHTML = '<div class="file-tree-empty">正在加载文件列表...</div>';
    try {
        const data = await api.fileTree(workspacePath);
        fileTreeEl.innerHTML = '';
        if (data.tree) {
            renderTreeNode(data.tree, fileTreeEl);
        }
    } catch (err) {
        fileTreeEl.innerHTML = `<div class="file-tree-empty" style="color:var(--error)">文件列表加载失败: ${escapeHtml(err.message || '未知错误')}</div>`;
    }
}

// ==================== Init ====================

export function init() {
    // Add refresh button to sidebar header
    const header = document.querySelector('.sidebar-header');
    if (header) {
        const refreshBtn = document.createElement('button');
        refreshBtn.className = 'sidebar-toggle';
        refreshBtn.title = '刷新文件树';
        refreshBtn.textContent = '🔄';
        refreshBtn.addEventListener('click', loadTree);
        header.insertBefore(refreshBtn, header.querySelector('.sidebar-toggle'));
    }

    // Workspace switch
    if (workspaceBtn) {
        workspaceBtn.addEventListener('click', () => {
            const path = workspaceInput?.value?.trim() || '.';
            setWorkspace(path);
        });
    }
    if (workspaceInput) {
        workspaceInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                const path = workspaceInput.value.trim() || '.';
                setWorkspace(path);
            }
        });
    }

    // Auto-load on init
    loadTree();
}

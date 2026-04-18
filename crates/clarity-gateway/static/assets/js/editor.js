/**
 * Clarity Editor Module - Monaco Editor + Multi-tab management
 */

import { store, openTab, closeTab, getActiveTab } from './store.js';
import * as api from './api.js';

// ==================== DOM Refs ====================

const editorContainer = document.getElementById('editor-container');
const tabsEl = document.getElementById('editor-tabs');

// ==================== Monaco ====================

let monaco = null;
let editor = null;

const LANG_MAP = {
    rs: 'rust', js: 'javascript', ts: 'typescript', jsx: 'javascript', tsx: 'typescript',
    html: 'html', htm: 'html', css: 'css', scss: 'scss', sass: 'sass',
    json: 'json', py: 'python', go: 'go', md: 'markdown',
    toml: 'ini', yaml: 'yaml', yml: 'yaml', xml: 'xml',
    c: 'c', cpp: 'cpp', cc: 'cpp', h: 'cpp', hpp: 'cpp',
    java: 'java', cs: 'csharp', sh: 'shell', ps1: 'powershell',
    sql: 'sql', php: 'php', rb: 'ruby', kt: 'kotlin',
    swift: 'swift', r: 'r', dart: 'dart',
};

function detectLanguage(path) {
    const ext = path.split('.').pop()?.toLowerCase() || '';
    return LANG_MAP[ext] || 'plaintext';
}

export async function init() {
    // Load Monaco from CDN with retry
    const monacoUrl = 'https://cdn.jsdelivr.net/npm/monaco-editor@0.45.0/min/vs';
    const script = document.createElement('script');
    script.src = `${monacoUrl}/loader.js`;
    script.onload = () => {
        window.require.config({ paths: { vs: monacoUrl } });
        window.require(['vs/editor/editor.main'], () => {
            monaco = window.monaco;
            defineClarityTheme();
            createEditor();
            setupKeyboardShortcuts();
        });
    };
    script.onerror = () => {
        showEditorError('编辑器加载失败', '请检查网络连接，或尝试刷新页面。如果持续失败，可以在聊天中继续与 AI 对话。');
    };
    document.head.appendChild(script);

    renderTabs();
}

function defineClarityTheme() {
    if (!monaco) return;
    monaco.editor.defineTheme('clarity-dark', {
        base: 'vs-dark',
        inherit: true,
        rules: [
            { token: 'comment', foreground: '6A9955', fontStyle: 'italic' },
            { token: 'keyword', foreground: 'C586C0' },
            { token: 'identifier', foreground: '9CDCFE' },
            { token: 'string', foreground: 'CE9178' },
            { token: 'number', foreground: 'B5CEA8' },
            { token: 'operator', foreground: 'D4D4D4' },
            { token: 'type', foreground: '4EC9B0' },
            { token: 'function', foreground: 'DCDCAA' },
            { token: 'variable', foreground: '9CDCFE' },
            { token: 'macro', foreground: '4EC9B0' },
            { token: 'tag', foreground: '569CD6' },
            { token: 'attribute.name', foreground: '9CDCFE' },
            { token: 'attribute.value', foreground: 'CE9178' },
            { token: 'delimiter', foreground: 'D4D4D4' },
        ],
        colors: {
            'editor.background': '#111118',
            'editor.foreground': '#e8e8ef',
            'editorLineNumber.foreground': '#5a5a6a',
            'editorLineNumber.activeForeground': '#8a8a9a',
            'editor.selectionBackground': '#6366f155',
            'editor.selectionHighlightBackground': '#6366f122',
            'editor.inactiveSelectionBackground': '#6366f133',
            'editor.wordHighlightBackground': '#6366f122',
            'editor.lineHighlightBackground': '#ffffff08',
            'editorCursor.foreground': '#6366f1',
            'editorWhitespace.foreground': '#333344',
            'editorIndentGuide.background': '#22222e',
            'editorIndentGuide.activeBackground': '#333344',
        }
    });
}

function showEditorError(title, hint) {
    editorContainer.innerHTML = `
        <div class="editor-empty">
            <div class="editor-empty-icon">⚠️</div>
            <div class="editor-empty-title">${title}</div>
            <div class="editor-empty-hint">${hint}</div>
        </div>
    `;
}

function createEditor() {
    if (!monaco) return;
    editorContainer.innerHTML = '';
    editor = monaco.editor.create(editorContainer, {
        value: '',
        language: 'plaintext',
        theme: 'clarity-dark',
        automaticLayout: true,
        fontSize: 14,
        fontFamily: "'JetBrains Mono', monospace",
        minimap: { enabled: true, scale: 1 },
        scrollBeyondLastLine: false,
        padding: { top: 16 },
        readOnly: false,
        renderWhitespace: 'selection',
        smoothScrolling: true,
        cursorBlinking: 'smooth',
        bracketPairColorization: { enabled: true },
        guides: { bracketPairs: true, indentation: true },
    });

    editor.onDidChangeModelContent(() => {
        const tab = getActiveTab();
        if (tab && !tab.dirty) {
            tab.dirty = true;
            renderTabs();
        }
    });
}

// ==================== Tab Management ====================

function renderTabs() {
    if (!tabsEl) return;
    tabsEl.innerHTML = '';

    for (const tab of store.tabs) {
        const el = document.createElement('div');
        el.className = `tab${tab.active ? ' active' : ''}`;
        el.innerHTML = `
            <span class="tab-icon">📄</span>
            <span class="tab-name">${escapeHtml(tab.name)}</span>
            ${tab.dirty ? '<span class="tab-dirty"></span>' : ''}
            <span class="tab-close" data-tab-id="${tab.id}">×</span>
        `;
        el.addEventListener('click', (e) => {
            if (e.target.classList.contains('tab-close')) {
                e.stopPropagation();
                closeTab(tab.id);
                renderTabs();
                updateEditor();
            } else {
                store.tabs.forEach(t => t.active = false);
                tab.active = true;
                store.activeTabId = tab.id;
                renderTabs();
                updateEditor();
            }
        });
        tabsEl.appendChild(el);
    }

    // New tab button
    const newBtn = document.createElement('div');
    newBtn.className = 'tab-new';
    newBtn.textContent = '+';
    newBtn.title = '新标签';
    newBtn.addEventListener('click', () => {
        openTab('untitled', '', 'plaintext');
        renderTabs();
        updateEditor();
    });
    tabsEl.appendChild(newBtn);
}

function updateEditor() {
    const tab = getActiveTab();
    if (!tab || !editor) {
        editorContainer.innerHTML = `
            <div class="editor-empty">
                <div class="editor-empty-icon">📁</div>
                <div class="editor-empty-title">选择文件开始编辑</div>
                <div class="editor-empty-hint">从左侧文件树点击文件，或在聊天中让 AI 帮你创建</div>
            </div>
        `;
        return;
    }

    if (!editor) {
        createEditor();
    }

    editor.setValue(tab.content);
    monaco.editor.setModelLanguage(editor.getModel(), tab.language);
    editor.focus();
}

// ==================== File Operations ====================

export async function openFile(path) {
    try {
        const data = await api.fileRead(path);
        const content = data.content || '';
        const lang = detectLanguage(path);
        openTab(path, content, lang);
        renderTabs();
        updateEditor();
    } catch (err) {
        console.error('Failed to open file:', err);
        import('./app.js').then(m => m.toast('打开文件失败: ' + err.message, 'error'));
    }
}

export async function saveActiveFile() {
    const tab = getActiveTab();
    if (!tab || !editor) return;
    try {
        const content = editor.getValue();
        await api.fileWrite(tab.path, content);
        tab.content = content;
        tab.dirty = false;
        renderTabs();
        import('./app.js').then(m => m.toast('文件已保存', 'success'));
    } catch (err) {
        console.error('Failed to save file:', err);
        import('./app.js').then(m => m.toast('保存失败: ' + err.message, 'error'));
    }
}

function escapeHtml(text) {
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// Keyboard shortcuts
export function setupKeyboardShortcuts() {
    document.addEventListener('keydown', (e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === 's') {
            e.preventDefault();
            saveActiveFile();
        }
        if ((e.ctrlKey || e.metaKey) && e.key === 'w') {
            e.preventDefault();
            const tab = getActiveTab();
            if (tab) {
                closeTab(tab.id);
                renderTabs();
                updateEditor();
            }
        }
    });
}

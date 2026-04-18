/**
 * Clarity App Entry Point
 * Features: Command Palette, Theme Switch, Onboarding, Model Popover
 */

import { store, loadSessions, addSession, getActiveSession } from './store.js';
import * as api from './api.js';
import * as chat from './chat.js';
import * as editor from './editor.js';
import * as files from './files.js';

// ==================== DOM Refs ====================

const app = document.getElementById('app');
const sidebar = document.getElementById('sidebar');
const sidebarToggle = document.getElementById('sidebar-toggle');
const chatPanel = document.getElementById('chat-panel');
const collapseChatBtn = document.getElementById('collapse-chat-btn');
const resizerLeft = document.getElementById('resizer-left');
const resizerRight = document.getElementById('resizer-right');
const settingsBtn = document.getElementById('settings-btn');
const settingsModal = document.getElementById('settings-modal');
const modalClose = document.getElementById('modal-close');
const configCancel = document.getElementById('config-cancel');
const configSave = document.getElementById('config-save');
const connectionStatus = document.getElementById('connection-status');

// New elements
const modelSelector = document.getElementById('model-selector');
const modelPopover = document.getElementById('model-popover');
const modelOptions = document.getElementById('model-options');
const modelDisplayName = document.getElementById('model-display-name');

const commandPalette = document.getElementById('command-palette');
const cmdInput = document.getElementById('cmd-input');
const cmdResults = document.getElementById('cmd-results');

const onboarding = document.getElementById('onboarding');

// ==================== State ====================

const AppState = { IDLE: 'idle', GENERATING: 'generating', OFFLINE: 'offline' };
let currentState = AppState.IDLE;

function setState(state) {
    currentState = state;
    app.dataset.state = state;
}

// ==================== Connection Health ====================

async function checkConnection() {
    const ok = await api.checkHealth();
    if (ok) {
        if (store.connectionStatus !== 'online') {
            store.connectionStatus = 'online';
            connectionStatus.textContent = '🟢';
            connectionStatus.title = '连接正常';
        }
    } else {
        if (store.connectionStatus !== 'offline') {
            store.connectionStatus = 'offline';
            connectionStatus.textContent = '🔴';
            connectionStatus.title = '未连接';
        }
    }
}

// ==================== Sidebar Toggle ====================

sidebarToggle.addEventListener('click', () => {
    store.sidebarCollapsed = !store.sidebarCollapsed;
    app.classList.toggle('sidebar-collapsed', store.sidebarCollapsed);
    sidebarToggle.textContent = store.sidebarCollapsed ? '▶' : '◀';
    sidebarToggle.title = store.sidebarCollapsed ? '展开侧边栏' : '收起侧边栏';
});

// ==================== Chat Panel Toggle ====================

collapseChatBtn.addEventListener('click', () => {
    store.chatCollapsed = !store.chatCollapsed;
    app.classList.toggle('chat-collapsed', store.chatCollapsed);
    collapseChatBtn.title = store.chatCollapsed ? '展开聊天' : '收起聊天';
});

// ==================== Resizers ====================

function setupResizer(resizer, targetCol, minWidth, side) {
    let isDragging = false;

    resizer.addEventListener('mousedown', (e) => {
        isDragging = true;
        resizer.classList.add('dragging');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', (e) => {
        if (!isDragging) return;
        const rect = app.getBoundingClientRect();
        let width;
        if (side === 'left') {
            width = e.clientX - rect.left;
        } else {
            width = rect.right - e.clientX;
        }
        width = Math.max(minWidth, width);
        app.style.gridTemplateColumns = side === 'left'
            ? `${width}px var(--resizer-width) 1fr var(--resizer-width) var(--chat-width)`
            : `var(--sidebar-width) var(--resizer-width) 1fr var(--resizer-width) ${width}px`;
    });

    document.addEventListener('mouseup', () => {
        if (isDragging) {
            isDragging = false;
            resizer.classList.remove('dragging');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
        }
    });
}

setupResizer(resizerLeft, 1, 160, 'left');
setupResizer(resizerRight, 5, 280, 'right');

// ==================== Model Popover (fixes overflow issue) ====================

const MODELS = [
    { id: 'kimi-code', name: 'Kimi Code', badge: '推荐' },
    { id: 'moonshot', name: 'Moonshot', badge: '' },
    { id: 'openai', name: 'OpenAI', badge: '' },
    { id: 'deepseek', name: 'DeepSeek', badge: '' },
    { id: 'anthropic', name: 'Claude', badge: '' },
    { id: 'kalosm', name: 'Kalosm (本地)', badge: '本地' },
];

let selectedModel = 'kimi-code';

function renderModelOptions() {
    modelOptions.innerHTML = MODELS.map(m => `
        <div class="model-option ${m.id === selectedModel ? 'active' : ''}" data-model="${m.id}">
            <span class="model-option-name">${m.name}</span>
            ${m.badge ? `<span class="model-option-badge">${m.badge}</span>` : ''}
        </div>
    `).join('');

    modelOptions.querySelectorAll('.model-option').forEach(el => {
        el.addEventListener('click', async () => {
            selectedModel = el.dataset.model;
            modelDisplayName.textContent = MODELS.find(m => m.id === selectedModel)?.name || '自动';
            document.getElementById('model-display').textContent = MODELS.find(m => m.id === selectedModel)?.name || '自动选择模型';
            closeModelPopover();
            try {
                await api.switchProvider(selectedModel);
                store.config.provider = selectedModel;
                toast(`已切换到 ${modelDisplayName.textContent}`, 'success');
            } catch (err) {
                toast('切换模型失败', 'error');
            }
            renderModelOptions();
        });
    });
}

function openModelPopover() {
    renderModelOptions();
    modelPopover.style.display = 'block';
}

function closeModelPopover() {
    modelPopover.style.display = 'none';
}

modelSelector?.addEventListener('click', (e) => {
    e.stopPropagation();
    openModelPopover();
});

modelPopover?.addEventListener('click', (e) => {
    if (e.target === modelPopover) closeModelPopover();
});

// ==================== Settings Modal ====================

function openSettings() {
    settingsModal.style.display = 'flex';
    loadConfigIntoForm();
}

function closeSettings() {
    settingsModal.style.display = 'none';
}

settingsBtn.addEventListener('click', openSettings);
modalClose.addEventListener('click', closeSettings);
configCancel.addEventListener('click', closeSettings);
settingsModal.addEventListener('click', (e) => {
    if (e.target === settingsModal) closeSettings();
});

async function loadConfigIntoForm() {
    try {
        const data = await api.getConfig();
        if (data.config) {
            document.getElementById('config-provider').value = data.config.provider || '';
            document.getElementById('config-api-key').value = '';
            document.getElementById('config-base-url').value = data.config.base_url || '';
            document.getElementById('config-model').value = data.config.model || '';
        }
    } catch (e) {
        console.error('Failed to load config:', e);
    }
}

configSave.addEventListener('click', async () => {
    const status = document.getElementById('config-status');
    const provider = document.getElementById('config-provider').value;
    const apiKey = document.getElementById('config-api-key').value;
    const baseUrl = document.getElementById('config-base-url').value || null;
    const model = document.getElementById('config-model').value || null;

    if (!provider || !apiKey) {
        status.textContent = '请选择服务商并输入 API 密钥';
        status.className = 'form-status error';
        return;
    }

    status.textContent = '保存中...';
    status.className = 'form-status';

    try {
        await api.setConfig({ provider, api_key: apiKey, base_url: baseUrl, model });
        status.textContent = '保存成功';
        status.className = 'form-status success';
        store.config.provider = provider;
        store.config.apiKeyMasked = apiKey.slice(0, 4) + '****' + apiKey.slice(-4);
        selectedModel = provider;
        modelDisplayName.textContent = MODELS.find(m => m.id === provider)?.name || provider;
        setTimeout(closeSettings, 800);
    } catch (e) {
        status.textContent = '保存失败: ' + e.message;
        status.className = 'form-status error';
    }
});

// ==================== Provider / Model Loading ====================

async function loadProvidersAndModels() {
    try {
        const configData = await api.getConfig();
        if (configData.config) {
            store.config = { ...store.config, ...configData.config };
            selectedModel = configData.config.provider || 'kimi-code';
            modelDisplayName.textContent = MODELS.find(m => m.id === selectedModel)?.name || '自动';
        }

        const modelsData = await api.getModels();
        if (modelsData.models) {
            store.models = modelsData.models;
        }

        const providers = ['kimi-code', 'moonshot', 'openai', 'deepseek', 'anthropic', 'kalosm'];
        const configProvider = document.getElementById('config-provider');
        if (configProvider) {
            configProvider.innerHTML = providers.map(p =>
                `<option value="${p}">${MODELS.find(m => m.id === p)?.name || p}</option>`
            ).join('');
        }
    } catch (e) {
        console.error('Failed to load providers/models:', e);
    }
}

// ==================== Theme Switch ====================

const THEMES = { dark: 'dark', light: 'light' };
let currentTheme = localStorage.getItem('clarity-theme') || 'dark';

function applyTheme(theme) {
    currentTheme = theme;
    document.documentElement.dataset.theme = theme;
    localStorage.setItem('clarity-theme', theme);

    // Update Monaco theme if editor is ready
    if (window.monaco?.editor) {
        window.monaco.editor.setTheme(theme === 'dark' ? 'clarity-dark' : 'vs');
    }

    // Update button states
    document.querySelectorAll('.theme-btn').forEach(btn => {
        btn.classList.toggle('active', btn.dataset.theme === theme);
    });
}

document.querySelectorAll('.theme-btn').forEach(btn => {
    btn.addEventListener('click', () => applyTheme(btn.dataset.theme));
});

// ==================== Command Palette ====================

const COMMANDS = [
    { id: 'new-chat', name: '新建对话', icon: '💬', key: 'Ctrl+N', action: () => { addSession('新对话'); chat.renderMessages(); closeCommandPalette(); } },
    { id: 'focus-input', name: '聚焦输入框', icon: '⌨️', key: 'Ctrl+K', action: () => { document.getElementById('chat-input')?.focus(); closeCommandPalette(); } },
    { id: 'send-message', name: '发送消息', icon: '➡️', key: 'Ctrl+Enter', action: () => { document.getElementById('send-btn')?.click(); closeCommandPalette(); } },
    { id: 'toggle-sidebar', name: '切换文件栏', icon: '📂', key: '', action: () => { sidebarToggle.click(); closeCommandPalette(); } },
    { id: 'toggle-chat', name: '切换聊天栏', icon: '💬', key: '', action: () => { collapseChatBtn.click(); closeCommandPalette(); } },
    { id: 'save-file', name: '保存文件', icon: '💾', key: 'Ctrl+S', action: () => { editor.saveActiveFile(); closeCommandPalette(); } },
    { id: 'close-tab', name: '关闭当前标签', icon: '❌', key: 'Ctrl+W', action: () => { /* delegated to editor.js */ closeCommandPalette(); } },
    { id: 'open-settings', name: '打开设置', icon: '⚙️', key: '', action: () => { openSettings(); closeCommandPalette(); } },
    { id: 'switch-model', name: '切换 AI 模型', icon: '🤖', key: '', action: () => { openModelPopover(); closeCommandPalette(); } },
    { id: 'toggle-theme', name: '切换深色/浅色主题', icon: '🌙', key: '', action: () => { applyTheme(currentTheme === 'dark' ? 'light' : 'dark'); closeCommandPalette(); } },
    { id: 'help', name: '查看快捷键帮助', icon: '❓', key: '', action: () => { toast('快捷键: Ctrl+Shift+P 命令面板 | Ctrl+Enter 发送 | Ctrl+S 保存 | Ctrl+K 聚焦输入', 'info', 5000); closeCommandPalette(); } },
];

function openCommandPalette() {
    commandPalette.style.display = 'flex';
    cmdInput.value = '';
    cmdInput.focus();
    renderCommands('');
}

function closeCommandPalette() {
    commandPalette.style.display = 'none';
}

function renderCommands(query) {
    const q = query.toLowerCase().trim();
    const filtered = q ? COMMANDS.filter(c => c.name.toLowerCase().includes(q)) : COMMANDS;

    if (filtered.length === 0) {
        cmdResults.innerHTML = '<div class="cmd-empty">未找到命令</div>';
        return;
    }

    cmdResults.innerHTML = `
        <div class="cmd-group">
            <div class="cmd-group-label">可用命令</div>
            ${filtered.map((c, i) => `
                <div class="cmd-item ${i === 0 ? 'selected' : ''}" data-cmd="${c.id}">
                    <span class="cmd-item-icon">${c.icon}</span>
                    <span class="cmd-item-name">${c.name}</span>
                    ${c.key ? `<span class="cmd-item-key">${c.key}</span>` : ''}
                </div>
            `).join('')}
        </div>
    `;

    cmdResults.querySelectorAll('.cmd-item').forEach(el => {
        el.addEventListener('click', () => {
            const cmd = COMMANDS.find(c => c.id === el.dataset.cmd);
            if (cmd) cmd.action();
        });
    });
}

cmdInput?.addEventListener('input', () => renderCommands(cmdInput.value));

cmdInput?.addEventListener('keydown', (e) => {
    const items = cmdResults.querySelectorAll('.cmd-item');
    let selected = Array.from(items).findIndex(el => el.classList.contains('selected'));

    if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (items.length > 0) {
            items[selected]?.classList.remove('selected');
            selected = (selected + 1) % items.length;
            items[selected].classList.add('selected');
            items[selected].scrollIntoView({ block: 'nearest' });
        }
    } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (items.length > 0) {
            items[selected]?.classList.remove('selected');
            selected = selected <= 0 ? items.length - 1 : selected - 1;
            items[selected].classList.add('selected');
            items[selected].scrollIntoView({ block: 'nearest' });
        }
    } else if (e.key === 'Enter') {
        e.preventDefault();
        const cmd = COMMANDS.find(c => c.id === items[selected]?.dataset.cmd);
        if (cmd) cmd.action();
    } else if (e.key === 'Escape') {
        closeCommandPalette();
    }
});

commandPalette?.addEventListener('click', (e) => {
    if (e.target === commandPalette) closeCommandPalette();
});

// ==================== Onboarding ====================

const ONBOARDING_KEY = 'clarity_onboarded_v2';

function initOnboarding() {
    if (localStorage.getItem(ONBOARDING_KEY)) return;
    if (!onboarding) return;

    const steps = onboarding.querySelectorAll('.onboarding-step');
    const dotsContainer = document.getElementById('onboarding-dots');
    const prevBtn = document.getElementById('onboarding-prev');
    const nextBtn = document.getElementById('onboarding-next');
    const finishBtn = document.getElementById('onboarding-finish');
    let currentStep = 0;
    const total = steps.length;

    // Create dots
    dotsContainer.innerHTML = Array.from({ length: total }, (_, i) =>
        `<div class="onboarding-dot ${i === 0 ? 'active' : ''}"></div>`
    ).join('');
    const dots = dotsContainer.querySelectorAll('.onboarding-dot');

    function showStep(n) {
        steps.forEach((s, i) => s.classList.toggle('active', i === n));
        dots.forEach((d, i) => d.classList.toggle('active', i === n));
        prevBtn.style.display = n === 0 ? 'none' : 'inline-block';
        nextBtn.style.display = n === total - 1 ? 'none' : 'inline-block';
        finishBtn.style.display = n === total - 1 ? 'inline-block' : 'none';
    }

    nextBtn.addEventListener('click', () => { if (currentStep < total - 1) { currentStep++; showStep(currentStep); } });
    prevBtn.addEventListener('click', () => { if (currentStep > 0) { currentStep--; showStep(currentStep); } });
    finishBtn.addEventListener('click', () => {
        localStorage.setItem(ONBOARDING_KEY, 'true');
        onboarding.style.display = 'none';
    });

    onboarding.style.display = 'flex';
    showStep(0);
}

// ==================== Keyboard Shortcuts ====================

document.addEventListener('keydown', (e) => {
    // Command Palette: F1 or Ctrl+Shift+P
    if (e.key === 'F1' || (e.ctrlKey && e.shiftKey && e.key === 'P')) {
        e.preventDefault();
        openCommandPalette();
        return;
    }

    // Escape closes any overlay
    if (e.key === 'Escape') {
        if (commandPalette?.style.display === 'flex') { closeCommandPalette(); return; }
        if (modelPopover?.style.display === 'block') { closeModelPopover(); return; }
        if (settingsModal?.style.display === 'flex') { closeSettings(); return; }
        if (onboarding?.style.display === 'flex') { onboarding.style.display = 'none'; return; }
    }

    // Ctrl+K: Focus chat input
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        document.getElementById('chat-input')?.focus();
    }
});

// ==================== Toast ====================

export function toast(message, type = 'info', duration = 3000) {
    const container = document.getElementById('toast-container');
    if (!container) return;
    const el = document.createElement('div');
    el.className = `toast ${type}`;
    el.textContent = message;
    container.appendChild(el);
    setTimeout(() => {
        el.style.opacity = '0';
        el.style.transform = 'translateX(100%)';
        setTimeout(() => el.remove(), 300);
    }, duration);
}

// ==================== Initialization ====================

async function init() {
    // Apply saved theme
    applyTheme(currentTheme);

    // Load persisted sessions
    loadSessions();
    if (store.sessions.length === 0) {
        addSession('新对话');
    }

    // Check connection
    await checkConnection();
    setInterval(checkConnection, 10000);

    // Load providers/models
    await loadProvidersAndModels();

    // Initialize sub-modules
    chat.init();
    editor.init();
    files.init();

    // Global toast container
    const toastContainer = document.createElement('div');
    toastContainer.className = 'toast-container';
    toastContainer.id = 'toast-container';
    document.body.appendChild(toastContainer);

    // Onboarding for first-time users
    initOnboarding();

    console.log('🜁 Clarity initialized');
}

init().catch(console.error);

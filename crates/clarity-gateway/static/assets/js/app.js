/**
 * Clarity App Entry Point
 * Features: Command Palette, Theme Switch, Onboarding, Model Popover
 */

import { store, addSession, getActiveSession } from './store.js';
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
    { id: 'local', name: 'Local (GGUF)', badge: '本地' },
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
    try {
        const modeData = await api.getApprovalMode();
        if (modeData.mode) {
            document.getElementById('config-approval-mode').value = modeData.mode;
        }
    } catch (e) {
        console.error('Failed to load approval mode:', e);
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
        const approvalMode = document.getElementById('config-approval-mode').value;
        await api.setApprovalMode(approvalMode);
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

        const providers = ['kimi-code', 'moonshot', 'openai', 'deepseek', 'anthropic', 'local'];
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
    { id: 'run-parallel', name: '并行执行子代理', icon: '🚀', key: '', action: () => { openParallelModal(); closeCommandPalette(); } },
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

// ==================== Parallel Execution Modal ====================

const parallelModal = document.getElementById('parallel-modal');
const parallelModalClose = document.getElementById('parallel-modal-close');
const parallelCancel = document.getElementById('parallel-cancel');
const parallelRun = document.getElementById('parallel-run');
const parallelAddTask = document.getElementById('parallel-add-task');
const parallelTasks = document.getElementById('parallel-tasks');
const parallelConcurrency = document.getElementById('parallel-concurrency');
const parallelStatus = document.getElementById('parallel-status');

function openParallelModal() {
    parallelModal.style.display = 'flex';
    parallelStatus.textContent = '';
}

function closeParallelModal() {
    parallelModal.style.display = 'none';
}

parallelModalClose?.addEventListener('click', closeParallelModal);
parallelCancel?.addEventListener('click', closeParallelModal);
parallelModal?.addEventListener('click', (e) => {
    if (e.target === parallelModal) closeParallelModal();
});

parallelAddTask?.addEventListener('click', () => {
    const rows = parallelTasks.querySelectorAll('.parallel-task-row');
    const idx = rows.length;
    const div = document.createElement('div');
    div.className = 'parallel-task-row';
    div.dataset.index = idx;
    div.innerHTML = `
        <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px;">
            <label style="font-size:12px;">任务 ${idx + 1}</label>
            <button class="parallel-remove-task" style="background:none;border:none;color:var(--text-secondary);cursor:pointer;font-size:14px;">×</button>
        </div>
        <div class="form-group" style="margin-bottom:8px;">
            <label style="font-size:12px;">类型</label>
            <input type="text" class="parallel-type" placeholder="coder / explore / plan" value="coder" style="width:100%;padding:6px 10px;border-radius:6px;border:1px solid var(--border);background:var(--surface);color:var(--text);">
        </div>
        <div class="form-group" style="margin-bottom:8px;">
            <label style="font-size:12px;">任务描述</label>
            <textarea class="parallel-prompt" rows="2" placeholder="输入任务描述..." style="width:100%;padding:6px 10px;border-radius:6px;border:1px solid var(--border);background:var(--surface);color:var(--text);resize:vertical;"></textarea>
        </div>
    `;
    parallelTasks.appendChild(div);
    div.querySelector('.parallel-remove-task')?.addEventListener('click', () => {
        div.remove();
        // renumber
        parallelTasks.querySelectorAll('.parallel-task-row').forEach((row, i) => {
            const lbl = row.querySelector('label');
            if (lbl) lbl.textContent = `任务 ${i + 1}`;
        });
    });
});

parallelRun?.addEventListener('click', async () => {
    const rows = parallelTasks.querySelectorAll('.parallel-task-row');
    const tasks = [];
    for (const row of rows) {
        const type = row.querySelector('.parallel-type')?.value.trim();
        const prompt = row.querySelector('.parallel-prompt')?.value.trim();
        if (!type || !prompt) continue;
        tasks.push({ agent_type: type, prompt });
    }
    if (tasks.length === 0) {
        parallelStatus.textContent = '请至少填写一个任务';
        return;
    }

    parallelStatus.textContent = '正在执行...';
    parallelRun.disabled = true;
    try {
        const result = await api.runParallel(tasks, parseInt(parallelConcurrency.value, 10));
        closeParallelModal();
        // Show result in chat
        const lines = [`🚀 并行执行完成 | 成功率: ${(result.success_rate * 100).toFixed(0)}% | 耗时: ${result.total_elapsed_ms}ms`];
        if (result.results?.length) {
            lines.push('');
            lines.push('✅ 成功结果:');
            for (const r of result.results) {
                lines.push(`• ${r.agent_id} (${r.agent_type}): ${r.summary}`);
            }
        }
        if (result.failures?.length) {
            lines.push('');
            lines.push('❌ 失败任务:');
            for (const f of result.failures) {
                lines.push(`• ${f.task_id}: ${f.error}`);
            }
        }
        chat.addSystemMessage(lines.join('\n'));
    } catch (e) {
        parallelStatus.textContent = `执行失败: ${e.message}`;
    } finally {
        parallelRun.disabled = false;
    }
});

// ==================== Tasks Modal ====================

const tasksModal = document.getElementById('tasks-modal');
const tasksModalClose = document.getElementById('tasks-modal-close');
const tasksRefresh = document.getElementById('tasks-refresh');
const tasksCloseBtn = document.getElementById('tasks-close');
const tasksList = document.getElementById('tasks-list');
const taskBadge = document.getElementById('task-badge');

function openTasksModal() {
    tasksModal.style.display = 'flex';
    renderTasksList();
}

function closeTasksModal() {
    tasksModal.style.display = 'none';
}

tasksModalClose?.addEventListener('click', closeTasksModal);
tasksCloseBtn?.addEventListener('click', closeTasksModal);
tasksModal?.addEventListener('click', (e) => {
    if (e.target === tasksModal) closeTasksModal();
});
tasksRefresh?.addEventListener('click', renderTasksList);

async function renderTasksList() {
    if (!tasksList) return;
    tasksList.innerHTML = '<div style="text-align:center;color:var(--text-secondary);padding:24px;">正在加载...</div>';
    try {
        const data = await api.listTasks();
        const tasks = data?.tasks || [];
        if (tasks.length === 0) {
            tasksList.innerHTML = '<div style="text-align:center;color:var(--text-secondary);padding:24px;">暂无后台任务</div>';
            return;
        }

        const isTerminal = (s) => s === 'Completed' || s === 'Failed' || s === 'Cancelled';
        const statusIcon = (s) => {
            if (s === 'Completed') return '✅';
            if (s === 'Running') return '🔄';
            if (s === 'Failed') return '❌';
            if (s === 'Cancelled') return '🚫';
            return '⏳';
        };

        const rows = tasks.map(t => {
            const date = new Date(t.created_at * 1000).toLocaleString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
            const canCancel = !isTerminal(t.status);
            return `
                <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-bottom:1px solid var(--border);gap:12px;">
                    <div style="flex:1;min-width:0;">
                        <div style="font-weight:500;font-size:13px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${t.name}</div>
                        <div style="font-size:11px;color:var(--text-secondary);margin-top:2px;">${statusIcon(t.status)} ${t.status} · ${date}</div>
                    </div>
                    <div style="display:flex;gap:6px;flex-shrink:0;">
                        ${canCancel ? `<button class="task-cancel-btn" data-id="${t.task_id}" style="padding:4px 10px;border-radius:4px;border:1px solid var(--error);background:transparent;color:var(--error);font-size:12px;cursor:pointer;">取消</button>` : ''}
                        <button class="task-detail-btn" data-id="${t.task_id}" style="padding:4px 10px;border-radius:4px;border:1px solid var(--border);background:var(--surface);color:var(--text);font-size:12px;cursor:pointer;">详情</button>
                    </div>
                </div>
            `;
        }).join('');

        tasksList.innerHTML = `<div style="border:1px solid var(--border);border-radius:8px;overflow:hidden;">${rows}</div>`;

        tasksList.querySelectorAll('.task-cancel-btn').forEach(btn => {
            btn.addEventListener('click', async () => {
                const id = btn.dataset.id;
                btn.disabled = true;
                btn.textContent = '...';
                try {
                    await api.cancelTask(id);
                    toast(`已取消任务 ${id.slice(0, 8)}...`, 'info');
                    renderTasksList();
                } catch (e) {
                    toast(`取消失败: ${e.message}`, 'error');
                    btn.disabled = false;
                    btn.textContent = '取消';
                }
            });
        });

        tasksList.querySelectorAll('.task-detail-btn').forEach(btn => {
            btn.addEventListener('click', async () => {
                const id = btn.dataset.id;
                try {
                    const detail = await api.getTask(id);
                    const lines = [
                        `ID: ${detail.task_id}`,
                        `名称: ${detail.name}`,
                        `状态: ${detail.status}`,
                        `提示词: ${detail.prompt}`,
                    ];
                    if (detail.result) {
                        lines.push(`结果: ${detail.result.output?.slice(0, 500) || '无输出'}`);
                        lines.push(`耗时: ${detail.result.elapsed_ms}ms`);
                    }
                    alert(lines.join('\n'));
                } catch (e) {
                    toast(`获取详情失败: ${e.message}`, 'error');
                }
            });
        });
    } catch (e) {
        tasksList.innerHTML = `<div style="text-align:center;color:var(--error);padding:24px;">加载失败: ${e.message}</div>`;
    }
}

// Task badge polling
taskBadge?.addEventListener('click', openTasksModal);

async function refreshTaskBadge() {
    try {
        const data = await api.listTasks();
        const tasks = data?.tasks || [];
        const running = tasks.filter(t => t.status === 'Running').length;
        if (running > 0) {
            taskBadge.textContent = running;
            taskBadge.style.display = 'inline-block';
        } else {
            taskBadge.style.display = 'none';
        }
    } catch (e) {
        taskBadge.style.display = 'none';
    }
}
refreshTaskBadge();
setInterval(refreshTaskBadge, 5000);

// ==================== Initialization ====================

async function init() {
    // Apply saved theme
    applyTheme(currentTheme);

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

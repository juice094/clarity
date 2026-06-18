/**
 * Clarity Chat Module - SSE / WebSocket streaming, message rendering, tool cards
 */

import { store, addMessage, getActiveSession, addSession, loadSessions } from './store.js';
import * as api from './api.js';
import { toast } from './app.js';
import * as ws from './ws-client.js';
import { renderWireMessage, resetRenderer, setCurrentAssistantBubble } from './wire-render.js';

// ==================== DOM Refs ====================

const messagesEl = document.getElementById('chat-messages');
const inputEl = document.getElementById('chat-input');
const sendBtn = document.getElementById('send-btn');
const newChatBtn = document.getElementById('new-chat-btn');
const newThreadBtn = document.getElementById('new-thread-btn');
const threadListEl = document.getElementById('thread-list');
const modelDisplay = document.getElementById('model-display');
const tokenDisplay = document.getElementById('token-display');

// ==================== Markdown (lightweight) ====================

function escapeHtml(text) {
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function renderMarkdown(text) {
    let html = escapeHtml(text);
    // Code blocks
    html = html.replace(/```(\w*)?\n([\s\S]*?)```/g, (_, lang, code) => {
        return `<pre><code class="language-${lang || 'text'}">${code.replace(/</g, '&lt;').replace(/>/g, '&gt;')}</code></pre>`;
    });
    // Inline code
    html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
    // Bold
    html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
    // Italic
    html = html.replace(/\*([^*]+)\*/g, '<em>$1</em>');
    // Headers
    html = html.replace(/^#### (.*$)/gim, '<h4>$1</h4>');
    html = html.replace(/^### (.*$)/gim, '<h3>$1</h3>');
    html = html.replace(/^## (.*$)/gim, '<h2>$1</h2>');
    html = html.replace(/^# (.*$)/gim, '<h1>$1</h1>');
    // Blockquote
    html = html.replace(/^\> (.*$)/gim, '<blockquote>$1</blockquote>');
    // Lists
    html = html.replace(/^- (.*$)/gim, '<li>$1</li>');
    // Horizontal rule
    html = html.replace(/^---$/gim, '<hr>');
    // Links
    html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank">$1</a>');
    // Paragraphs
    const parts = html.split(/\n\n+/);
    html = parts.map(p => {
        const trimmed = p.trim();
        if (!trimmed) return '';
        if (trimmed.startsWith('<') && !trimmed.startsWith('<li>')) return trimmed;
        if (trimmed.startsWith('<li>')) return `<ul>${trimmed}</ul>`;
        return `<p>${trimmed.replace(/\n/g, '<br>')}</p>`;
    }).join('');
    return html;
}

// ==================== Message Rendering ====================

function createMessageElement(role, content = '', isStreaming = false) {
    const wrapper = document.createElement('div');
    wrapper.className = `message ${role}${isStreaming ? ' streaming' : ''}`;

    const avatar = document.createElement('div');
    avatar.className = 'message-avatar';
    avatar.textContent = role === 'user' ? 'U' : 'C';

    const contentDiv = document.createElement('div');
    contentDiv.className = 'message-content';

    const bubble = document.createElement('div');
    bubble.className = 'message-bubble';
    bubble.innerHTML = isStreaming ? '<span class="cursor"></span>' : renderMarkdown(content);

    const meta = document.createElement('div');
    meta.className = 'message-meta';
    if (isStreaming) {
        meta.innerHTML = '<span class="loading-dots"><span></span><span></span><span></span></span>';
    } else {
        meta.innerHTML = `<span>${new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>`;
    }

    contentDiv.appendChild(bubble);
    contentDiv.appendChild(meta);
    wrapper.appendChild(avatar);
    wrapper.appendChild(contentDiv);

    messagesEl.appendChild(wrapper);

    // Scroll to bottom
    messagesEl.scrollTop = messagesEl.scrollHeight;

    return { wrapper, bubble, meta };
}

function updateMessageBubble(bubble, text, isStreaming = false) {
    bubble.innerHTML = renderMarkdown(text) + (isStreaming ? '<span class="cursor"></span>' : '');
    messagesEl.scrollTop = messagesEl.scrollHeight;
}

function finalizeMessage(bubble, meta, text) {
    bubble.innerHTML = renderMarkdown(text);
    meta.innerHTML = `<span>${new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
        <div class="message-actions">
            <button class="message-action-btn" data-action="copy">复制</button>
            <button class="message-action-btn" data-action="retry">重试</button>
        </div>`;
    messagesEl.scrollTop = messagesEl.scrollHeight;
}

// ==================== Tool Call Cards ====================

function createToolCard(container, id, name, args = '{}') {
    const card = document.createElement('div');
    card.className = 'tool-call-card';
    card.dataset.toolId = id;

    const header = document.createElement('div');
    header.className = 'tool-call-header';
    header.innerHTML = `
        <span class="tool-icon">🔧</span>
        <span class="tool-name">${escapeHtml(name)}</span>
        <span class="tool-status executing">执行中</span>
    `;

    const body = document.createElement('div');
    body.className = 'tool-call-body';
    body.textContent = typeof args === 'string' ? args : JSON.stringify(args, null, 2);

    header.addEventListener('click', () => {
        body.classList.toggle('collapsed');
    });

    card.appendChild(header);
    card.appendChild(body);
    container.appendChild(card);
    messagesEl.scrollTop = messagesEl.scrollHeight;
    return card;
}

function updateToolResult(card, result) {
    const status = card.querySelector('.tool-status');
    const body = card.querySelector('.tool-call-body');
    status.className = 'tool-status done';
    status.textContent = '完成';
    body.textContent = typeof result === 'string' ? result : JSON.stringify(result, null, 2);
}

// ==================== Chat Logic ====================

let abortController = null;

async function sendMessage() {
    const text = inputEl.value.trim();
    if (!text || store.isGenerating) return;

    const session = getActiveSession();
    if (!session) return;

    // Add user message
    addMessage(session.id, { role: 'user', content: text });
    createMessageElement('user', text);

    inputEl.value = '';
    inputEl.style.height = 'auto';
    store.isGenerating = true;
    sendBtn.disabled = true;
    sendBtn.innerHTML = '⏹';

    // Create assistant placeholder for both WebSocket and SSE paths.
    const { bubble, meta } = createMessageElement('assistant', '', true);

    // Prefer WebSocket if available; fall back to SSE otherwise.
    if (ws.isOpen()) {
        try {
            resetRenderer();
            setCurrentAssistantBubble(bubble, meta);
            ws.sendChat(text, true);
            return;
        } catch (err) {
            console.warn('WebSocket send failed, falling back to SSE:', err);
        }
    }

    await sendMessageSSE(session, bubble, meta);
}

async function sendMessageSSE(session, bubble, meta) {
    let assistantText = '';
    const toolCards = new Map();

    abortController = new AbortController();

    try {
        const messages = session.messages.map(m => ({ role: m.role, content: m.content }));
        const stream = store.currentThreadId
            ? api.streamThreadChat(store.currentThreadId, { model: store.currentModel, messages })
            : api.streamChat({ model: store.currentModel, messages });

        for await (const event of stream) {
            if (abortController.signal.aborted) break;

            if (event.type === 'delta') {
                if (event.content) {
                    assistantText += event.content;
                    updateMessageBubble(bubble, assistantText, true);
                }
                if (event.toolCalls) {
                    for (const tc of event.toolCalls) {
                        const tcId = tc.id || tc.index;
                        if (!toolCards.has(tcId)) {
                            const args = tc.function?.arguments || '{}';
                            const card = createToolCard(
                                bubble.parentElement,
                                tcId,
                                tc.function?.name || 'unknown',
                                args
                            );
                            toolCards.set(tcId, card);
                        }
                    }
                }
            } else if (event.type === 'tool_result') {
                const card = toolCards.get(event.id);
                if (card) updateToolResult(card, event.result);
            } else if (event.type === 'step_begin') {
                // Show a subtle indicator that a tool step has started
                const stepIndicator = document.createElement('div');
                stepIndicator.className = 'step-indicator';
                stepIndicator.textContent = `🔧 正在执行: ${event.toolName || '工具'}`;
                stepIndicator.style.cssText = 'font-size:12px;color:var(--text-tertiary);margin:4px 0;padding:4px 8px;background:var(--bg-primary);border-radius:4px;';
                bubble.parentElement.appendChild(stepIndicator);
                messagesEl.scrollTop = messagesEl.scrollHeight;
            } else if (event.type === 'finish') {
                break;
            }
        }

        finalizeMessage(bubble, meta, assistantText);
        addMessage(session.id, { role: 'assistant', content: assistantText });
    } catch (err) {
        bubble.innerHTML = `<p style="color:var(--error)">❌ 请求失败: ${escapeHtml(err.message)}</p>`;
        meta.innerHTML = `<span>${new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>`;
        console.error('Chat error:', err);
    } finally {
        store.isGenerating = false;
        sendBtn.disabled = false;
        sendBtn.innerHTML = `
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"></line><polygon points="22 2 15 22 11 13 2 9 22 2"></polygon></svg>
        `;
        abortController = null;
    }
}

function stopGeneration() {
    if (abortController) {
        abortController.abort();
    }
}

export function resetInputState() {
    store.isGenerating = false;
    sendBtn.disabled = false;
    sendBtn.innerHTML = `
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"></line><polygon points="22 2 15 22 11 13 2 9 22 2"></polygon></svg>
    `;
}

// ==================== Input Handling ====================

inputEl.addEventListener('input', () => {
    inputEl.style.height = 'auto';
    inputEl.style.height = Math.min(inputEl.scrollHeight, 120) + 'px';
});

inputEl.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        if (store.isGenerating) {
            stopGeneration();
        } else {
            sendMessage();
        }
    }
});

sendBtn.addEventListener('click', () => {
    if (store.isGenerating) {
        stopGeneration();
    } else {
        sendMessage();
    }
});

// ==================== New Chat / Thread ====================

newChatBtn.addEventListener('click', () => {
    store.currentThreadId = null;
    addSession('新对话');
    updateUrlThreadId(null);
    renderMessages();
});

newThreadBtn?.addEventListener('click', () => {
    startNewThread();
});

// ==================== Suggestion Chips ====================

messagesEl.addEventListener('click', (e) => {
    const chip = e.target.closest('.chip');
    if (chip) {
        const preset = chip.dataset.preset;
        if (preset) {
            inputEl.value = preset;
            sendMessage();
        }
    }
});

// ==================== Render Messages ====================

export function renderMessages() {
    messagesEl.innerHTML = '';
    const session = getActiveSession();
    if (!session || session.messages.length === 0) {
        messagesEl.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">🤖</div>
                <h2>今天想聊点什么？</h2>
                <p>Clarity 是你的 AI 编程助手</p>
                <div class="suggestion-chips">
                    <button class="chip" data-preset="简单自我介绍">自我介绍</button>
                    <button class="chip" data-preset="用 Rust 写一个快速排序">写快排</button>
                    <button class="chip" data-preset="分析当前项目结构">分析项目</button>
                </div>
            </div>
        `;
        return;
    }
    for (const msg of session.messages) {
        createMessageElement(msg.role, msg.content);
    }
}

// ==================== System Messages ====================

export function addSystemMessage(text) {
    const session = getActiveSession();
    if (!session) return;
    addMessage(session.id, { role: 'system', content: text });
    createMessageElement('system', text);
}

// ==================== Thread Management ====================

function updateUrlThreadId(id) {
    const url = new URL(window.location.href);
    if (id) {
        url.searchParams.set('thread_id', id);
    } else {
        url.searchParams.delete('thread_id');
    }
    history.replaceState(null, '', url.toString());
}

function loadThread(thread) {
    if (!thread || !thread.id) return;
    store.currentThreadId = thread.id;
    updateUrlThreadId(thread.id);

    let session = store.sessions.find(s => s.threadId === thread.id);
    if (!session) {
        session = addSession(thread.title || '(untitled)', thread.id);
    } else {
        store.activeSessionId = session.id;
    }

    const history = thread.history || thread.messages || [];
    session.messages = history.map(m => ({ role: m.role, content: m.content || '' }));
    session.title = thread.title || '(untitled)';
    session.updatedAt = thread.updated_at ? new Date(thread.updated_at).getTime() : Date.now();

    renderMessages();
}

async function startNewThread() {
    if (store.isGenerating) return;
    try {
        const thread = await api.createThread();
        loadThread(thread);
        await renderThreadList();
    } catch (err) {
        console.warn('Failed to create thread, falling back to local session:', err);
        toast('Thread API 不可用，使用本地会话', 'info');
        store.currentThreadId = null;
        updateUrlThreadId(null);
        addSession('新对话');
        renderMessages();
    }
}

async function switchThread(id) {
    if (!id || id === store.currentThreadId) return;
    try {
        const thread = await api.getThread(id, true);
        loadThread(thread);
        renderThreadList();
    } catch (err) {
        console.error('Failed to switch thread:', err);
        toast('切换会话失败', 'error');
    }
}

export async function renderThreadList() {
    if (!threadListEl) return;
    threadListEl.innerHTML = '<div class="session-item">加载中...</div>';
    try {
        const data = await api.listThreads();
        const threads = data?.threads || [];
        if (threads.length === 0) {
            threadListEl.innerHTML = '<div class="session-item" style="color:var(--text-tertiary);cursor:default;">暂无对话</div>';
            return;
        }

        threadListEl.innerHTML = '';
        for (const t of threads) {
            const el = document.createElement('div');
            el.className = `session-item thread-item ${t.id === store.currentThreadId ? 'active' : ''}`;
            el.title = t.title || '(untitled)';

            const updated = t.updated_at ? new Date(t.updated_at) : null;
            const timeStr = updated && !isNaN(updated)
                ? updated.toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
                : '';

            el.innerHTML = `
                <span class="thread-title">${escapeHtml(t.title || '(untitled)')}</span>
                <span class="thread-time">${escapeHtml(timeStr)}</span>
            `;
            el.addEventListener('click', () => switchThread(t.id));
            threadListEl.appendChild(el);
        }
    } catch (err) {
        console.error('Failed to render thread list:', err);
        threadListEl.innerHTML = `<div class="session-item" style="color:var(--error);cursor:default;">加载失败</div>`;
    }
}

async function initializeThread() {
    const params = new URLSearchParams(window.location.search);
    const threadId = params.get('thread_id');

    if (threadId) {
        try {
            const thread = await api.getThread(threadId, true);
            loadThread(thread);
            return;
        } catch (err) {
            console.warn('Failed to load thread from URL:', err);
            toast('无法加载指定会话', 'error');
        }
    }

    try {
        const thread = await api.createThread();
        loadThread(thread);
    } catch (err) {
        console.warn('Thread API unavailable, using local session fallback:', err);
        toast('Thread API 不可用，使用本地会话', 'info');
        store.currentThreadId = null;
        updateUrlThreadId(null);
        loadSessions();
        if (store.sessions.length === 0) {
            addSession('新对话');
        }
        renderMessages();
    }
}

// ==================== Init ====================

export async function init() {
    await initializeThread();
    renderThreadList();

    // Connect WebSocket and route wire messages to the renderer.
    ws.on('wire_message', renderWireMessage);
    ws.on('error_response', (data) => {
        resetInputState();
        addSystemMessage(`❌ 请求失败: ${data.error || 'unknown error'}`);
    });
    ws.connect();

    // Keep connection alive.
    setInterval(() => ws.ping(), 30000);

    // Watch session changes
    let lastSessionId = store.activeSessionId;
    setInterval(() => {
        if (store.activeSessionId !== lastSessionId) {
            lastSessionId = store.activeSessionId;
            renderMessages();
        }
    }, 100);
}

// ==================== Exports for wire-render.js ====================

export { createMessageElement, updateMessageBubble, finalizeMessage, createToolCard, updateToolResult };

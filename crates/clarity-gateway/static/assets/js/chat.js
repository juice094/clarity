/**
 * Clarity Chat Module - SSE streaming, message rendering, tool cards
 */

import { store, addMessage, getActiveSession, addSession } from './store.js';
import * as api from './api.js';
import { toast } from './app.js';

// ==================== DOM Refs ====================

const messagesEl = document.getElementById('chat-messages');
const inputEl = document.getElementById('chat-input');
const sendBtn = document.getElementById('send-btn');
const newChatBtn = document.getElementById('new-chat-btn');
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

    // Create assistant placeholder
    const { bubble, meta } = createMessageElement('assistant', '', true);
    let assistantText = '';
    const toolCards = new Map();

    abortController = new AbortController();

    try {
        const messages = session.messages.map(m => ({ role: m.role, content: m.content }));
        const stream = api.streamChat({
            model: store.currentModel,
            messages,
        });

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

// ==================== New Chat ====================

newChatBtn.addEventListener('click', () => {
    addSession('新对话');
    renderMessages();
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

// ==================== Init ====================

export function init() {
    renderMessages();

    // Watch session changes
    let lastSessionId = store.activeSessionId;
    setInterval(() => {
        if (store.activeSessionId !== lastSessionId) {
            lastSessionId = store.activeSessionId;
            renderMessages();
        }
    }, 100);
}

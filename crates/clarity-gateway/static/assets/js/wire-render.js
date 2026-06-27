/**
 * WireMessage renderer for the Gateway Web IDE.
 *
 * Takes `WireMessagePayload` objects from the WebSocket and maps them to
 * the existing chat DOM. Keeps a reference to the current assistant bubble
 * so that streaming content can be appended in place.
 */

import { store, addMessage, getActiveSession } from './store.js';
import {
    createMessageElement,
    updateMessageBubble,
    finalizeMessage,
    createToolCard,
    updateToolResult,
    addSystemMessage,
    resetInputState,
} from './chat.js';
import { toast } from './app.js';

/** @type {{ wrapper: HTMLElement, bubble: HTMLElement, meta: HTMLElement } | null} */
let currentAssistant = null;

/** @type {{ bubble: HTMLElement, meta: HTMLElement } | null} */
let externalAssistant = null;

/** @type {Map<string, HTMLElement>} */
const toolCards = new Map();

/**
 * Set an existing assistant bubble to be used by the renderer.
 * Called by chat.js before sending a message.
 * @param {HTMLElement} bubble
 * @param {HTMLElement} meta
 */
export function setCurrentAssistantBubble(bubble, meta) {
    externalAssistant = { bubble, meta };
}

/**
 * Reset the renderer state at the start of a new turn.
 */
export function resetRenderer() {
    currentAssistant = externalAssistant;
    toolCards.clear();
}

/**
 * Render a single WireMessage payload.
 * @param {import('./types/protocol.ts').WireMessagePayload} payload
 */
export function renderWireMessage(payload) {
    if (!payload || typeof payload !== 'object') return;

    switch (payload.type) {
        case 'turn_begin':
            handleTurnBegin(payload);
            break;
        case 'step_begin':
            handleStepBegin(payload);
            break;
        case 'content_part':
            handleContentPart(payload);
            break;
        case 'draft_event':
            handleDraftEvent(payload);
            break;
        case 'tool_call':
            handleToolCall(payload);
            break;
        case 'tool_call_progress':
            handleToolCallProgress(payload);
            break;
        case 'tool_result':
            handleToolResult(payload);
            break;
        case 'status_update':
            handleStatusUpdate(payload);
            break;
        case 'turn_end':
            handleTurnEnd();
            break;
        case 'usage':
            handleUsage(payload);
            break;
        case 'view_state_update':
            handleViewStateUpdate(payload);
            break;
        case 'compaction_begin':
            handleStatusUpdate({ message: 'Compacting context...' });
            break;
        case 'compaction_end':
            removeTransientStatus();
            break;
        case 'plan_step_begin':
        case 'plan_step_end':
        case 'plan_step_skipped':
            handlePlanStep(payload);
            break;
        case 'thread_active':
        case 'thread_list':
        case 'thread_created':
        case 'thread_updated':
            // Thread management is handled separately by the session list.
            console.debug('Thread wire message received:', payload);
            break;
        default:
            console.debug('Unhandled wire message type:', payload.type);
    }
}

function ensureAssistantBubble() {
    if (!currentAssistant) {
        currentAssistant = createMessageElement('assistant', '', true);
    }
    return currentAssistant;
}

function handleTurnBegin(payload) {
    store.isGenerating = true;
    ensureAssistantBubble();
}

function handleContentPart(payload) {
    if (!payload.text) return;
    const { bubble, meta } = ensureAssistantBubble();
    const currentText = bubble.dataset.rawText || '';
    const newText = currentText + payload.text;
    bubble.dataset.rawText = newText;
    updateMessageBubble(bubble, newText, true);
    meta.innerHTML = '<span class="loading-dots"><span></span><span></span><span></span></span>';
}

function handleDraftEvent(payload) {
    const { event } = payload;
    if (!event) return;

    const { bubble } = ensureAssistantBubble();
    if (event.type === 'clear') {
        bubble.querySelector('.draft-indicator')?.remove();
    } else if (event.type === 'progress') {
        showTransientStatus(event.text || 'thinking...');
    } else if (event.type === 'content') {
        showTransientStatus(event.text || '', 'draft-content');
    }
}

function handleToolCall(payload) {
    const id = payload.id || `tool_${Date.now()}`;
    const name = payload.name || 'unknown';
    const args = payload.arguments || '{}';
    const card = createToolCard(currentAssistant?.bubble?.parentElement || document.getElementById('chat-messages'), id, name, args);
    toolCards.set(id, card);
}

function handleToolCallProgress(payload) {
    const name = payload.name || 'tool';
    const idx = payload.index || 0;
    const label = name ? `⚙ ${name} #${idx} assembling…` : `tool #${idx} assembling…`;
    showTransientStatus(label);
}

function handleToolResult(payload) {
    const card = toolCards.get(payload.id);
    if (card) {
        updateToolResult(card, payload.result);
    }
}

function handleStepBegin(payload) {
    showTransientStatus(`🔧 正在执行: ${payload.tool_name || '工具'}…`, 'step-indicator');
}

function handleStatusUpdate(payload) {
    showTransientStatus(payload.message || '', 'status-indicator');
}

function handleTurnEnd() {
    finalizeCurrentAssistant();
    resetInputState();
    resetRenderer();
    removeTransientStatus();
}

function handleUsage(payload) {
    const tokenDisplay = document.getElementById('token-display');
    if (tokenDisplay && payload.total_tokens) {
        tokenDisplay.textContent = `${payload.total_tokens} tokens`;
    }
}

function handleViewStateUpdate(payload) {
    const turn = payload.turn;
    if (!turn) return;
    const statusEl = document.getElementById('connection-status');
    if (!statusEl) return;

    if (turn === 'loading') {
        statusEl.textContent = '⚪';
        statusEl.title = '生成中...';
    } else if (turn === 'compacting') {
        statusEl.textContent = '🔵';
        statusEl.title = '压缩上下文中...';
    } else {
        statusEl.textContent = store.connectionStatus === 'online' ? '🟢' : '🔴';
        statusEl.title = store.connectionStatus === 'online' ? '连接正常' : '未连接';
    }
}

function handlePlanStep(payload) {
    // Plan step updates are currently displayed as transient status rows.
    // A dedicated plan panel can be wired here in the future.
    const labels = {
        plan_step_begin: '开始',
        plan_step_end: '完成',
        plan_step_skipped: '跳过',
    };
    const stepId = payload.step_id || '';
    const label = labels[payload.type] || payload.type;
    showTransientStatus(`📋 计划步骤 ${label}: ${stepId}`, 'plan-step-indicator');
}

function finalizeCurrentAssistant() {
    if (!currentAssistant) return;
    const { bubble, meta } = currentAssistant;
    const text = bubble.dataset.rawText || '';
    finalizeMessage(bubble, meta, text);

    const session = getActiveSession();
    if (session && text) {
        addMessage(session.id, { role: 'assistant', content: text });
    }
}

/** @type {HTMLElement | null} */
let transientStatusEl = null;

function showTransientStatus(text, className = 'transient-status') {
    if (!text) return;
    const container = document.getElementById('chat-messages');
    if (!container) return;

    removeTransientStatus();

    transientStatusEl = document.createElement('div');
    transientStatusEl.className = className;
    transientStatusEl.textContent = text;
    transientStatusEl.style.cssText =
        'font-size:12px;color:var(--text-tertiary);margin:4px 0;padding:4px 8px;background:var(--bg-primary);border-radius:4px;';
    container.appendChild(transientStatusEl);
    container.scrollTop = container.scrollHeight;
}

function removeTransientStatus() {
    if (transientStatusEl && transientStatusEl.parentElement) {
        transientStatusEl.remove();
    }
    transientStatusEl = null;
}

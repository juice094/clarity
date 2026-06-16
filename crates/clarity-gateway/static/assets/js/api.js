/**
 * Clarity API Client - All backend communication
 */

const BASE_URL = '';

async function request(path, options = {}) {
    const url = path.startsWith('http') ? path : `${BASE_URL}${path}`;
    const resp = await fetch(url, {
        headers: { 'Content-Type': 'application/json', ...options.headers },
        ...options,
    });
    if (!resp.ok) {
        const text = await resp.text().catch(() => '');
        throw new Error(`HTTP ${resp.status}: ${text}`);
    }
    return resp.json().catch(() => null);
}

// ==================== Health ====================

export async function checkHealth() {
    try {
        const resp = await fetch(`${BASE_URL}/health`, { method: 'GET' });
        return resp.ok;
    } catch {
        return false;
    }
}

// ==================== Chat Completions (SSE) ====================

async function* parseSSE(resp) {
    const reader = resp.body.getReader();
    const decoder = new TextDecoder('utf-8');
    let buffer = '';

    try {
        while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            buffer += decoder.decode(value, { stream: true });
            const lines = buffer.split('\n');
            buffer = lines.pop();

            for (const line of lines) {
                const trimmed = line.trim();
                if (!trimmed.startsWith('data:')) continue;
                const data = trimmed.slice(5).trim();
                if (data === '[DONE]' || !data) continue;

                try {
                    const json = JSON.parse(data);

                    // OpenAI-compatible delta
                    const choice = json.choices?.[0];
                    if (choice?.delta) {
                        yield { type: 'delta', content: choice.delta.content || '',
                            toolCalls: choice.delta.tool_calls };
                    }
                    if (choice?.finish_reason) {
                        yield { type: 'finish', reason: choice.finish_reason };
                    }

                    // Clarity extension events
                    if (json.object === 'clarity.event') {
                        if (json.type === 'tool_result') {
                            yield { type: 'tool_result', id: json.id, result: json.result };
                        } else if (json.type === 'step_begin') {
                            yield { type: 'step_begin', toolName: json.tool_name };
                        }
                    }
                } catch (e) {
                    console.warn('SSE parse error:', e, data);
                }
            }
        }
    } finally {
        reader.releaseLock();
    }
}

export async function* streamChat({ model, messages }) {
    const resp = await fetch(`${BASE_URL}/v1/chat/completions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model, messages, stream: true }),
    });

    if (!resp.ok) {
        const text = await resp.text().catch(() => '');
        throw new Error(`HTTP ${resp.status}: ${text}`);
    }

    yield* parseSSE(resp);
}

// Non-streaming chat completion
export async function completeChat({ model, messages }) {
    return request('/v1/chat/completions', {
        method: 'POST',
        body: JSON.stringify({ model, messages, stream: false }),
    });
}

// ==================== V2 Threads ====================

export async function listThreads(limit = 20) {
    return request(`/api/v2/threads?limit=${encodeURIComponent(limit)}`);
}

export async function createThread(title = null) {
    const body = title ? { title } : {};
    return request('/api/v2/threads', {
        method: 'POST',
        body: JSON.stringify(body),
    });
}

export async function getThread(id, includeHistory = false) {
    return request(`/api/v2/threads/${encodeURIComponent(id)}?include_history=${includeHistory}`);
}

export async function* streamThreadChat(threadId, { model, messages }) {
    const resp = await fetch(`${BASE_URL}/api/v2/threads/${encodeURIComponent(threadId)}/chat`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model, messages, stream: true }),
    });

    if (!resp.ok) {
        const text = await resp.text().catch(() => '');
        throw new Error(`HTTP ${resp.status}: ${text}`);
    }

    yield* parseSSE(resp);
}

export async function completeThreadChat(threadId, { model, messages }) {
    return request(`/api/v2/threads/${encodeURIComponent(threadId)}/chat`, {
        method: 'POST',
        body: JSON.stringify({ model, messages, stream: false }),
    });
}

// ==================== File System API ====================

export async function fileTree(path = '.') {
    const params = new URLSearchParams({ path });
    return request(`/api/files/tree?${params}`);
}

export async function fileRead(path, offset, limit) {
    const params = new URLSearchParams({ path });
    if (offset !== undefined) params.set('offset', String(offset));
    if (limit !== undefined) params.set('limit', String(limit));
    return request(`/api/files/read?${params}`);
}

export async function fileWrite(path, content) {
    return request('/api/files/write', {
        method: 'POST',
        body: JSON.stringify({ path, content }),
    });
}

export async function fileGlob(pattern) {
    const params = new URLSearchParams({ pattern });
    return request(`/api/files/glob?${params}`);
}

// ==================== Admin API ====================

export async function getConfig() {
    return request('/api/config');
}

export async function setConfig(config) {
    return request('/api/config', {
        method: 'POST',
        body: JSON.stringify(config),
    });
}

export async function getModels() {
    return request('/api/models');
}

export async function switchProvider(provider) {
    return request('/api/provider', {
        method: 'POST',
        body: JSON.stringify({ provider }),
    });
}

export async function getStats() {
    return request('/api/stats');
}

export async function getTools() {
    return request('/api/tools');
}

export async function getApprovalMode() {
    return request('/api/approval-mode');
}

export async function setApprovalMode(mode) {
    return request('/api/approval-mode', {
        method: 'POST',
        body: JSON.stringify({ mode }),
    });
}

// ==================== Tasks ====================

export async function createTask(spec) {
    return request('/v1/tasks', {
        method: 'POST',
        body: JSON.stringify(spec),
    });
}

export async function getTask(id) {
    return request(`/v1/tasks/${id}`);
}

export async function cancelTask(id) {
    return fetch(`${BASE_URL}/v1/tasks/${id}`, { method: 'DELETE' });
}

export async function listTasks() {
    return request('/v1/tasks');
}

// ==================== Parallel Subagents ====================

export async function runParallel(tasks, maxConcurrency) {
    const body = { tasks };
    if (maxConcurrency !== undefined) body.max_concurrency = maxConcurrency;
    return request('/v1/parallel', {
        method: 'POST',
        body: JSON.stringify(body),
    });
}

/**
 * Clarity Store - Lightweight reactive state management (~150 lines)
 * Inspired by Vue 3 reactivity, but zero dependencies.
 */

const subscribers = new WeakMap();
const activeEffect = { current: null };

function track(target, key) {
    if (!activeEffect.current) return;
    let deps = subscribers.get(target);
    if (!deps) {
        deps = new Map();
        subscribers.set(target, deps);
    }
    let effects = deps.get(key);
    if (!effects) {
        effects = new Set();
        deps.set(key, effects);
    }
    effects.add(activeEffect.current);
}

function trigger(target, key) {
    const deps = subscribers.get(target);
    if (!deps) return;
    const effects = deps.get(key);
    if (effects) {
        effects.forEach(fn => fn());
    }
}

export function ref(value) {
    const obj = { _value: value };
    return new Proxy(obj, {
        get(target, key) {
            if (key === '_value' || key === 'value') {
                track(target, 'value');
                return target._value;
            }
            return target[key];
        },
        set(target, key, val) {
            if (key === '_value' || key === 'value') {
                if (target._value !== val) {
                    target._value = val;
                    trigger(target, 'value');
                }
                return true;
            }
            target[key] = val;
            return true;
        }
    });
}

export function reactive(obj) {
    return new Proxy(obj, {
        get(target, key) {
            track(target, key);
            const val = target[key];
            if (val !== null && typeof val === 'object') {
                return reactive(val);
            }
            return val;
        },
        set(target, key, val) {
            const old = target[key];
            if (old !== val) {
                target[key] = val;
                trigger(target, key);
            }
            return true;
        }
    });
}

export function computed(getter) {
    const r = ref(undefined);
    watchEffect(() => {
        r.value = getter();
    });
    return r;
}

export function watch(source, callback) {
    let oldValue;
    watchEffect(() => {
        const newValue = typeof source === 'function' ? source() : source.value;
        if (oldValue !== undefined) {
            callback(newValue, oldValue);
        }
        oldValue = newValue;
    });
}

export function watchEffect(fn) {
    activeEffect.current = fn;
    fn();
    activeEffect.current = null;
}

/**
 * Central App Store
 */
export const store = reactive({
    // UI State
    sidebarCollapsed: false,
    chatCollapsed: false,
    connectionStatus: 'connecting', // 'connecting' | 'online' | 'offline' | 'error'

    // Chat State
    sessions: [],
    activeSessionId: null,
    isGenerating: false,
    currentModel: 'auto',

    // Editor State
    tabs: [], // { id, path, name, content, language, dirty, active }
    activeTabId: null,

    // Config
    config: {
        provider: '',
        apiKeyMasked: '',
        baseUrl: null,
        model: null,
    },
    providers: [],
    models: [],
});

// Helpers
export function getActiveSession() {
    return store.sessions.find(s => s.id === store.activeSessionId);
}

export function getActiveTab() {
    return store.tabs.find(t => t.id === store.activeTabId);
}

export function addSession(title = 'New Chat') {
    const id = 'sess_' + Date.now().toString(36);
    const session = {
        id,
        title,
        messages: [],
        createdAt: Date.now(),
        updatedAt: Date.now(),
    };
    store.sessions.push(session);
    store.activeSessionId = id;
    return session;
}

export function addMessage(sessionId, message) {
    const session = store.sessions.find(s => s.id === sessionId);
    if (session) {
        session.messages.push(message);
        session.updatedAt = Date.now();
    }
}

export function openTab(path, content = '', language = 'plaintext') {
    const existing = store.tabs.find(t => t.path === path);
    if (existing) {
        store.activeTabId = existing.id;
        return existing;
    }
    const id = 'tab_' + Date.now().toString(36);
    const name = path.split('/').pop() || 'untitled';
    const tab = {
        id,
        path,
        name,
        content,
        language,
        dirty: false,
        active: true,
    };
    store.tabs.forEach(t => t.active = false);
    store.tabs.push(tab);
    store.activeTabId = id;
    return tab;
}

export function closeTab(id) {
    const idx = store.tabs.findIndex(t => t.id === id);
    if (idx === -1) return;
    store.tabs.splice(idx, 1);
    if (store.activeTabId === id) {
        const next = store.tabs[Math.min(idx, store.tabs.length - 1)];
        store.activeTabId = next ? next.id : null;
        if (next) next.active = true;
    }
}

// Persistence
const STORAGE_KEY = 'clarity_sessions_v2';

export function persistSessions() {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(store.sessions));
    } catch (e) {
        console.warn('Failed to persist sessions:', e);
    }
}

export function loadSessions() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (raw) {
            const sessions = JSON.parse(raw);
            store.sessions.splice(0, store.sessions.length, ...sessions);
            if (sessions.length > 0 && !store.activeSessionId) {
                store.activeSessionId = sessions[sessions.length - 1].id;
            }
        }
    } catch (e) {
        console.warn('Failed to load sessions:', e);
    }
}

// Auto-persist on session changes
let persistTimer = null;
watchEffect(() => {
    // Trigger re-run when sessions change
    const _ = store.sessions.length;
    if (persistTimer) clearTimeout(persistTimer);
    persistTimer = setTimeout(persistSessions, 500);
});

/**
 * Clarity Gateway WebSocket client.
 *
 * Connects to `/ws` and exposes a callback-based interface for the unified
 * `WsResponse` envelope. Streaming agent events are delivered as
 * `wire_message` payloads and routed through `wire-render.js`.
 */

import { store } from './store.js';

const WS_URL = `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/ws`;

/** @type {WebSocket | null} */
let socket = null;
let reconnectAttempts = 0;
const MAX_RECONNECT_ATTEMPTS = 5;
const RECONNECT_DELAY_MS = 2000;

/** @type {Map<string, Set<Function>>} */
const listeners = new Map();

/** @type {Promise<void> | null} */
let openPromise = null;
/** @type {(() => void) | null} */
let openResolve = null;
/** @type {((reason?: any) => void) | null} */
let openReject = null;

function resetOpenPromise() {
    openPromise = new Promise((resolve, reject) => {
        openResolve = resolve;
        openReject = reject;
    });
}

resetOpenPromise();

/**
 * Register an event handler.
 * @param {string} type - 'open' | 'close' | 'error' | 'welcome' | 'wire_message' | 'error_response' | 'history'
 * @param {Function} handler
 */
export function on(type, handler) {
    if (!listeners.has(type)) {
        listeners.set(type, new Set());
    }
    listeners.get(type).add(handler);
}

/**
 * Remove an event handler.
 * @param {string} type
 * @param {Function} handler
 */
export function off(type, handler) {
    listeners.get(type)?.delete(handler);
}

function emit(type, payload) {
    listeners.get(type)?.forEach(handler => {
        try {
            handler(payload);
        } catch (err) {
            console.error(`WebSocket handler error for ${type}:`, err);
        }
    });
}

export function connect() {
    if (socket?.readyState === WebSocket.OPEN || socket?.readyState === WebSocket.CONNECTING) {
        return;
    }

    store.connectionStatus = 'connecting';
    socket = new WebSocket(WS_URL);

    socket.addEventListener('open', () => {
        reconnectAttempts = 0;
        store.connectionStatus = 'online';
        openResolve?.();
        resetOpenPromise();
        emit('open');
    });

    socket.addEventListener('message', (event) => {
        let data;
        try {
            data = JSON.parse(event.data);
        } catch (err) {
            console.warn('Failed to parse WebSocket message:', event.data, err);
            return;
        }

        switch (data.type) {
            case 'welcome':
                store.sessionId = data.session_id;
                emit('welcome', data);
                break;
            case 'wire_message':
                emit('wire_message', data.payload);
                break;
            case 'history':
                emit('history', data.messages);
                break;
            case 'error':
                emit('error_response', data);
                break;
            case 'pong':
                break;
            default:
                console.debug('Unhandled WebSocket response type:', data.type);
        }
    });

    socket.addEventListener('close', () => {
        store.connectionStatus = 'offline';
        socket = null;
        emit('close');
        if (reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
            reconnectAttempts += 1;
            setTimeout(connect, RECONNECT_DELAY_MS);
        }
    });

    socket.addEventListener('error', (err) => {
        store.connectionStatus = 'error';
        openReject?.(err);
        resetOpenPromise();
        emit('error', err);
    });
}

/**
 * Wait for the WebSocket connection to open.
 * @returns {Promise<void>}
 */
export function waitForOpen() {
    if (socket?.readyState === WebSocket.OPEN) {
        return Promise.resolve();
    }
    return openPromise ?? Promise.resolve();
}

/**
 * Send a chat message over the WebSocket.
 * @param {string} message
 * @param {boolean} useWire - whether to request streaming wire events.
 */
export function sendChat(message, useWire = true) {
    if (socket?.readyState !== WebSocket.OPEN) {
        throw new Error('WebSocket is not open');
    }
    socket.send(JSON.stringify({ type: 'chat', message, use_wire: useWire }));
}

/**
 * Send a ping to keep the connection alive.
 */
export function ping() {
    if (socket?.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: 'ping' }));
    }
}

/**
 * Request conversation history for the current session.
 */
export function requestHistory() {
    if (socket?.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: 'get_history' }));
    }
}

/**
 * Close the WebSocket connection.
 */
export function close() {
    socket?.close();
    socket = null;
}

/**
 * @returns {boolean}
 */
export function isOpen() {
    return socket?.readyState === WebSocket.OPEN;
}

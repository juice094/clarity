<template>
  <div class="conversation-view">
    <!-- Sidebar (left) -->
    <aside class="sidebar">
      <div class="sidebar-header">Sessions</div>
      <div class="session-list">
        <div
          v-for="s in sessions"
          :key="s.id"
          class="session-item"
          :class="{ active: s.id === activeSessionId }"
          @click="activeSessionId = s.id"
        >
          {{ s.title }}
        </div>
      </div>
    </aside>

    <!-- Main chat area -->
    <main class="conv-main">
      <header class="conv-header">
        <span class="conv-title">{{ activeSession?.title || 'New Chat' }}</span>
        <button class="theme-toggle" @click="toggleTheme">
          {{ isDark ? 'Light' : 'Dark' }}
        </button>
      </header>

      <div class="messages">
        <div class="message-list-inner">
          <div
            v-for="(msg, i) in messages"
            :key="i"
            class="msg"
            :class="{ 'msg-user': msg.role === 'user', 'msg-assistant': msg.role === 'agent' }"
          >
            <div v-if="msg.role === 'agent'" class="msg-avatar">AI</div>
            <div class="bubble" :class="msg.role === 'user' ? 'role-user' : 'role-assistant'">
              <div class="blocks">{{ msg.content }}</div>
            </div>
          </div>
        </div>
      </div>

      <div class="composer-dock">
        <div class="composer-inner">
          <textarea
            v-model="input"
            class="composer-input"
            placeholder="Type a message..."
            @keydown.ctrl.enter="send"
          />
          <button class="send-btn" @click="send">Send</button>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

interface Message {
  role: 'user' | 'agent'
  content: string
}

interface Session {
  id: string
  title: string
  messages: Message[]
}

const isDark = ref(true)
const input = ref('')
const activeSessionId = ref('1')

const sessions = ref<Session[]>([
  { id: '1', title: 'General', messages: [
    { role: 'user', content: 'Hello, how are you?' },
    { role: 'agent', content: 'I am doing well, thank you! How can I help you today?' },
  ]},
  { id: '2', title: 'Coding', messages: [] },
])

const activeSession = computed(() =>
  sessions.value.find(s => s.id === activeSessionId.value)
)

const messages = computed(() => activeSession.value?.messages ?? [])

function send() {
  if (!input.value.trim()) return
  const session = activeSession.value
  if (!session) return
  session.messages.push({ role: 'user', content: input.value })
  input.value = ''
  // Simulate response
  setTimeout(() => {
    session.messages.push({
      role: 'agent',
      content: 'This is a placeholder response from the Tauri frontend.',
    })
  }, 500)
}

function toggleTheme() {
  isDark.value = !isDark.value
}
</script>

<style scoped>
.conversation-view {
  flex: 1;
  display: flex;
  flex-direction: row;
  min-height: 0;
  height: 100%;
  background: var(--Bg-Primary);
}

.sidebar {
  width: 260px;
  flex-shrink: 0;
  border-right: 0.5px solid var(--Separators-S1);
  background: var(--Bg-Primary);
  display: flex;
  flex-direction: column;
  padding: 16px;
}

.sidebar-header {
  font-size: 12px;
  font-weight: 500;
  color: var(--Labels-Secondary);
  text-transform: uppercase;
  margin-bottom: 12px;
}

.session-item {
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 14px;
  color: var(--Labels-Secondary);
  transition: background 0.15s ease;
}

.session-item:hover {
  background: var(--Fills-F1);
}

.session-item.active {
  background: var(--Fills-F2);
  color: var(--Labels-Primary);
}

.conv-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  height: 100%;
}

.conv-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 40px;
  padding: 0 16px;
  background: var(--Bg-Primary);
  flex-shrink: 0;
  border-bottom: 0.5px solid var(--Separators-S1);
}

.conv-title {
  font-size: 16px;
  line-height: 24px;
  color: var(--Labels-Primary);
}

.theme-toggle {
  padding: 4px 12px;
  border-radius: 6px;
  border: none;
  background: var(--Fills-F1);
  color: var(--Labels-Secondary);
  cursor: pointer;
  font-size: 12px;
}

.messages {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.message-list-inner {
  width: 100%;
  max-width: 800px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 16px 30px 4px;
  box-sizing: border-box;
}

.msg {
  display: flex;
  width: 100%;
  box-sizing: border-box;
}

.msg-user {
  flex-direction: row-reverse;
}

.msg-assistant {
  align-items: flex-start;
}

.msg-avatar {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 8px;
  background: var(--Colors-KMBlue);
  border-radius: 50%;
  color: white;
  font-size: 12px;
  font-weight: 500;
}

.bubble {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 4px;
  word-wrap: break-word;
}

.role-user {
  max-width: 80%;
  padding: 12px 16px;
  border-radius: 12px;
  background: var(--Fills-F2);
  color: var(--Labels-Primary);
}

.role-assistant {
  flex: 1;
  min-width: 0;
  padding: 12px 0 12px 16px;
  color: var(--Labels-Primary);
}

.blocks {
  display: flex;
  flex-direction: column;
  gap: 12px;
  font-size: 16px;
  line-height: 26px;
  min-width: 0;
}

.composer-dock {
  flex-shrink: 0;
  padding: 0 16px 16px;
  background: var(--Bg-Primary);
}

.composer-inner {
  width: 100%;
  max-width: 800px;
  margin: 0 auto;
  box-sizing: border-box;
  display: flex;
  gap: 8px;
  align-items: flex-end;
  padding: 12px 16px;
  border: 0.5px solid var(--Separators-S1);
  border-radius: 16px;
  background: var(--Bg-Secondary);
}

.composer-input {
  flex: 1;
  border: none;
  outline: none;
  background: transparent;
  resize: none;
  font-size: 14px;
  line-height: 20px;
  color: var(--Labels-Primary);
  font-family: inherit;
  min-height: 20px;
  max-height: 120px;
}

.composer-input::placeholder {
  color: var(--Labels-Tertiary);
}

.send-btn {
  padding: 6px 16px;
  border-radius: 10px;
  border: none;
  background: var(--Labels-Primary);
  color: var(--Bg-Primary);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
}

.send-btn:hover {
  opacity: 0.9;
}
</style>

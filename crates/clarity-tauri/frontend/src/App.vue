<template>
  <div class="app" :class="{ dark: isDark }">
    <ConversationView />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import ConversationView from './components/ConversationView.vue'

const isDark = ref(true)
</script>

<style>
.app {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: var(--Bg-Primary);
  color: var(--Labels-Primary);
  font-family: var(--Font-Family-Base);
}
</style>

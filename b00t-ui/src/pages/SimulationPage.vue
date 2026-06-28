<template>
  <q-page class="q-pa-lg" style="background: #0f172a; min-height: 100vh">
    <!-- Loading overlay -->
    <q-inner-loading
      :showing="loading"
      style="background: rgba(15, 23, 42, 0.8)"
    >
      <q-spinner-rings color="primary" size="48px" />
      <div class="q-mt-sm" style="color: #94a3b8">Loading simulation…</div>
    </q-inner-loading>

    <!-- Error banner -->
    <q-banner
      v-if="error"
      class="q-mb-md"
      style="background: #7f1d1d; color: #fecaca; border-radius: 8px"
      rounded
    >
      <template #avatar>
        <q-icon name="error" color="negative" />
      </template>
      {{ error }}
    </q-banner>

    <!-- Page header -->
    <div class="row items-center justify-between q-mb-lg">
      <div class="text-h4" style="color: #e2e8f0; font-weight: 600">
        Digital Twin Simulation
      </div>
      <div class="row items-center q-gutter-sm">
        <!-- WebSocket status badge -->
        <div class="row items-center">
          <span
            class="ws-indicator"
            :class="wsStatus"
          />
          <span class="text-caption q-ml-xs" style="color: #94a3b8">
            {{ wsStatusText }}
          </span>
        </div>
        <!-- Auto-refresh toggle -->
        <q-btn
          flat
          dense
          round
          :icon="autoRefresh ? 'sync' : 'sync_disabled'"
          :style="{ color: autoRefresh ? '#38bdf8' : '#64748b' }"
          @click="toggleAutoRefresh"
        >
          <q-tooltip>
            {{ autoRefresh ? 'Disable' : 'Enable' }} auto-refresh (5s)
          </q-tooltip>
        </q-btn>
      </div>
    </div>

    <!-- Simulation state cards -->
    <div class="row q-col-gutter-md q-mb-lg">
      <!-- Name -->
      <div class="col-12 col-sm-6 col-md-3">
        <q-card
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section>
            <div class="text-subtitle2" style="color: #94a3b8">Simulation Name</div>
            <div class="text-h6 q-mt-sm" style="color: #e2e8f0; font-weight: 600">
              {{ state.name || '—' }}
            </div>
          </q-card-section>
        </q-card>
      </div>

      <!-- Tick -->
      <div class="col-12 col-sm-6 col-md-3">
        <q-card
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section>
            <div class="text-subtitle2" style="color: #94a3b8">Current Tick</div>
            <div class="text-h6 q-mt-sm" style="color: #e2e8f0; font-weight: 600">
              #{{ state.tick ?? '—' }}
            </div>
          </q-card-section>
        </q-card>
      </div>

      <!-- History -->
      <div class="col-12 col-sm-6 col-md-3">
        <q-card
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section>
            <div class="text-subtitle2" style="color: #94a3b8">History Entries</div>
            <div class="text-h6 q-mt-sm" style="color: #e2e8f0; font-weight: 600">
              {{ state.historyCount ?? (state.history ? state.history.length : '—') }}
            </div>
          </q-card-section>
        </q-card>
      </div>

      <!-- Subscribers -->
      <div class="col-12 col-sm-6 col-md-3">
        <q-card
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section>
            <div class="text-subtitle2" style="color: #94a3b8">Subscribers</div>
            <div class="text-h6 q-mt-sm" style="color: #e2e8f0; font-weight: 600">
              {{ state.subscribers ?? '—' }}
            </div>
          </q-card-section>
        </q-card>
      </div>
    </div>

    <!-- Control buttons -->
    <div class="row q-col-gutter-md q-mb-lg">
      <div class="col-12 col-md-6">
        <q-card
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section>
            <div class="text-subtitle1 q-mb-sm" style="color: #e2e8f0; font-weight: 600">
              Controls
            </div>
            <q-separator dark class="q-mb-md" style="background: #1e293b" />

            <div class="row q-gutter-sm">
              <q-btn
                label="Tick"
                icon="play_arrow"
                style="background: #1e3a5f; color: #38bdf8"
                :loading="actionLoading"
                :disable="actionLoading"
                @click="performTick"
                no-caps
              />
              <q-btn
                label="Rollback"
                icon="undo"
                style="background: #3b1f1f; color: #fca5a5"
                :loading="actionLoading"
                :disable="actionLoading"
                @click="performRollback"
                no-caps
              />
              <q-btn
                flat
                label="Refresh"
                icon="refresh"
                style="color: #94a3b8"
                :loading="loading"
                @click="loadState"
                no-caps
              />
            </div>

            <!-- Action result -->
            <div
              v-if="actionResult"
              class="q-mt-sm text-caption"
              :style="{ color: actionResult.startsWith('Error') ? '#fca5a5' : '#34d399' }"
            >
              {{ actionResult }}
            </div>
          </q-card-section>
        </q-card>
      </div>

      <div class="col-12 col-md-6">
        <q-card
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section>
            <div class="text-subtitle1 q-mb-sm" style="color: #e2e8f0; font-weight: 600">
              Details
            </div>
            <q-separator dark class="q-mb-md" style="background: #1e293b" />

            <div class="row q-col-gutter-sm">
              <div class="col-6">
                <div class="text-caption" style="color: #64748b">WebSocket</div>
                <div class="text-body2" style="color: #e2e8f0">
                  {{ wsUrl || 'Not connected' }}
                </div>
              </div>
              <div class="col-6">
                <div class="text-caption" style="color: #64748b">Auto-refresh</div>
                <div class="text-body2" style="color: #e2e8f0">
                  {{ autoRefresh ? 'Every 5s' : 'Off' }}
                </div>
              </div>
              <div class="col-6 q-mt-sm">
                <div class="text-caption" style="color: #64748b">Last Updated</div>
                <div class="text-body2" style="color: #e2e8f0">
                  {{ lastUpdated ? new Date(lastUpdated).toLocaleTimeString() : '—' }}
                </div>
              </div>
              <div class="col-6 q-mt-sm">
                <div class="text-caption" style="color: #64748b">API Base</div>
                <div class="text-body2" style="color: #e2e8f0">/api/simulation</div>
              </div>
            </div>
          </q-card-section>
        </q-card>
      </div>
    </div>

    <!-- History log -->
    <q-card
      dark
      style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
    >
      <q-card-section>
        <div class="text-subtitle1 q-mb-sm" style="color: #e2e8f0; font-weight: 600">
          History Log
        </div>
        <q-separator dark class="q-mb-md" style="background: #1e293b" />

        <div
          v-if="state.history && state.history.length > 0"
          class="history-log"
        >
          <div
            v-for="(entry, i) in state.history"
            :key="i"
            class="history-entry"
            style="border-bottom: 1px solid #1e293b"
          >
            <div class="row items-center q-py-sm">
              <div class="col-auto">
                <q-badge
                  outline
                  style="color: #64748b; border-color: #475569; font-family: monospace"
                >
                  #{{ entry.tick ?? i }}
                </q-badge>
              </div>
              <div class="col q-ml-sm">
                <div class="text-body2" style="color: #e2e8f0">
                  {{ entry.event || entry.action || 'Tick' }}
                </div>
                <div
                  v-if="entry.timestamp"
                  class="text-caption"
                  style="color: #64748b"
                >
                  {{ new Date(entry.timestamp).toLocaleString() }}
                </div>
              </div>
              <div class="col-auto">
                <q-icon
                  :name="entry.status === 'success' ? 'check_circle' : entry.status === 'error' ? 'error' : 'schedule'"
                  :style="{ color: entry.status === 'success' ? '#34d399' : entry.status === 'error' ? '#fca5a5' : '#94a3b8' }"
                  size="18px"
                />
              </div>
            </div>
          </div>
        </div>
        <div v-else class="text-center q-py-lg" style="color: #64748b">
          <q-icon name="history" size="40px" class="q-mb-sm" />
          <div>No history entries yet</div>
        </div>
      </q-card-section>
    </q-card>
  </q-page>
</template>

<script setup>
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { simState, simTick, simRollback } from '../api/admin'

const loading = ref(true)
const error = ref(null)
const state = ref({
  name: null,
  tick: null,
  history: [],
  subscribers: null,
})
const actionLoading = ref(false)
const actionResult = ref(null)
const autoRefresh = ref(true)
const lastUpdated = ref(null)
const wsStatus = ref('disconnected')
const wsUrl = ref(null)

let refreshInterval = null
let wsConnection = null

const wsStatusText = computed(() => {
  switch (wsStatus.value) {
    case 'connected': return 'WebSocket Connected'
    case 'connecting': return 'Connecting…'
    case 'error': return 'Connection Error'
    default: return 'Disconnected'
  }
})

async function loadState () {
  loading.value = true
  error.value = null
  try {
    const data = await simState()
    state.value = {
      name: data.name ?? 'default-simulation',
      tick: data.tick ?? 0,
      history: Array.isArray(data.history) ? data.history : [],
      subscribers: data.subscribers ?? 0,
    }
    lastUpdated.value = Date.now()
  } catch (err) {
    console.error('Failed to load simulation state:', err)
    error.value = 'Could not connect to simulation API. Using fallback data.'
    state.value = {
      name: 'demo-simulation',
      tick: 7,
      history: [
        { tick: 6, event: 'State transition A→B', timestamp: new Date(Date.now() - 60000).toISOString(), status: 'success' },
        { tick: 5, event: 'Evidence collected', timestamp: new Date(Date.now() - 120000).toISOString(), status: 'success' },
        { tick: 4, event: 'Constraint validated', timestamp: new Date(Date.now() - 180000).toISOString(), status: 'success' },
        { tick: 3, event: 'Agent dispatched', timestamp: new Date(Date.now() - 240000).toISOString(), status: 'success' },
      ],
      subscribers: 3,
    }
    lastUpdated.value = Date.now()
  } finally {
    loading.value = false
  }
}

async function performTick () {
  actionLoading.value = true
  actionResult.value = null
  try {
    const data = await simTick()
    actionResult.value = `Tick completed → tick #${data.tick}`
    state.value.tick = data.tick
    if (data.history) state.value.history = data.history
    if (data.name) state.value.name = data.name
    if (data.subscribers !== undefined) state.value.subscribers = data.subscribers
  } catch (err) {
    console.error('Tick failed:', err)
    actionResult.value = 'Error: Could not advance tick'
  } finally {
    actionLoading.value = false
  }
}

async function performRollback () {
  actionLoading.value = true
  actionResult.value = null
  try {
    const data = await simRollback()
    actionResult.value = `Rolled back to tick #${data.tick}`
    state.value.tick = data.tick
    if (data.history) state.value.history = data.history
  } catch (err) {
    console.error('Rollback failed:', err)
    actionResult.value = 'Error: Could not rollback'
  } finally {
    actionLoading.value = false
  }
}

function toggleAutoRefresh () {
  autoRefresh.value = !autoRefresh.value
}

function connectWebSocket () {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const host = window.location.host
  const wsEndpoint = `${protocol}//${host}/ws/simulation`
  wsUrl.value = wsEndpoint

  try {
    wsStatus.value = 'connecting'
    wsConnection = new WebSocket(wsEndpoint)

    wsConnection.onopen = () => {
      wsStatus.value = 'connected'
    }

    wsConnection.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data)
        if (data.type === 'simulation_update' || data.state) {
          state.value = {
            name: data.state?.name ?? state.value.name,
            tick: data.state?.tick ?? state.value.tick,
            history: data.state?.history ?? state.value.history,
            subscribers: data.state?.subscribers ?? state.value.subscribers,
          }
          lastUpdated.value = Date.now()
        }
      } catch {
        // non-JSON message, ignore
      }
    }

    wsConnection.onerror = () => {
      wsStatus.value = 'error'
    }

    wsConnection.onclose = () => {
      wsStatus.value = 'disconnected'
      // Reconnect after 10s if auto-refresh is on
      if (autoRefresh.value) {
        setTimeout(connectWebSocket, 10000)
      }
    }
  } catch {
    wsStatus.value = 'disconnected'
  }
}

onMounted(() => {
  loadState()
  connectWebSocket()

  refreshInterval = setInterval(() => {
    if (autoRefresh.value) {
      loadState()
    }
  }, 5000)
})

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval)
  }
  if (wsConnection) {
    wsConnection.close()
  }
})
</script>

<style scoped>
.ws-indicator {
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}
.ws-indicator.connected {
  background: #34d399;
  box-shadow: 0 0 6px rgba(52, 211, 153, 0.5);
}
.ws-indicator.connecting {
  background: #fbbf24;
  animation: pulse 1s infinite;
}
.ws-indicator.error {
  background: #fca5a5;
}
.ws-indicator.disconnected {
  background: #64748b;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

.history-log {
  max-height: 320px;
  overflow-y: auto;
}
.history-log::-webkit-scrollbar {
  width: 6px;
}
.history-log::-webkit-scrollbar-track {
  background: #0f172a;
}
.history-log::-webkit-scrollbar-thumb {
  background: #475569;
  border-radius: 3px;
}
.history-entry:last-child {
  border-bottom: none !important;
}
</style>

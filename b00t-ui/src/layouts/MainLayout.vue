<template>
  <q-layout view="lHh Lpr lFf" class="dashboard-layout">
    <!-- ── Header ──────────────────────────────────────────── -->
    <q-header class="dashboard-header">
      <q-toolbar>
        <q-btn
          flat
          dense
          round
          icon="menu"
          aria-label="Toggle drawer"
          @click="drawerOpen = !drawerOpen"
          class="text-grey-4"
        />

        <q-toolbar-title class="text-grey-4">
          <span class="text-accent">b00t</span> Admin
        </q-toolbar-title>

        <q-badge
          v-if="store.health"
          :color="healthBadgeColor"
          outline
          class="q-px-sm q-py-xs"
        >
          {{ healthBadgeLabel }}
        </q-badge>
      </q-toolbar>
    </q-header>

    <!-- ── Left Sidebar (200px, dark) ─────────────────────── -->
    <q-drawer
      v-model="drawerOpen"
      :width="200"
      side="left"
      :breakpoint="500"
      bordered
      class="dashboard-sidebar"
    >
      <q-scroll-area style="height: 100%">
        <q-list padding dense class="text-grey-4">
          <template v-for="section in sections" :key="section.key">
            <!-- Section header — click to expand/collapse + activate -->
            <q-item
              clickable
              v-ripple
              :active="activeSection === section.key"
              active-class="text-accent sidebar-item-active"
              @click="toggleSection(section.key)"
              class="sidebar-header"
            >
              <q-item-section avatar class="sidebar-icon-col">
                <q-icon :name="section.icon" size="sm" />
              </q-item-section>
              <q-item-section>
                <q-item-label class="text-weight-medium text-body2">{{
                  section.label
                }}</q-item-label>
              </q-item-section>
              <q-item-section side>
                <q-icon
                  :name="chevronIcon(section.key)"
                  size="xs"
                  class="text-grey-6"
                />
              </q-item-section>
            </q-item>

            <!-- Child items (animated expand/collapse) -->
            <q-slide-transition>
              <div v-show="expandedSection === section.key">
                <q-item
                  v-for="child in section.children"
                  :key="child.key"
                  clickable
                  v-ripple
                  dense
                  class="sidebar-child q-pl-lg"
                  :active="activeChild === child.key"
                  active-class="text-accent"
                  @click="selectChild(section.key, child.key)"
                >
                  <q-item-section side class="sidebar-icon-col">
                    <q-icon :name="child.icon" size="xs" />
                  </q-item-section>
                  <q-item-section>
                    <q-item-label caption class="text-caption">{{
                      child.label
                    }}</q-item-label>
                  </q-item-section>
                </q-item>
              </div>
            </q-slide-transition>
          </template>
        </q-list>
      </q-scroll-area>
    </q-drawer>

    <!-- ── Main Content Panel ──────────────────────────────── -->
    <q-page-container class="dashboard-content">
      <!-- Pipeline -->
      <q-page v-if="activeSection === 'pipeline'" class="q-pa-lg">
        <div class="text-h5 text-accent q-mb-lg">Pipeline</div>
        <q-card dark bordered class="dashboard-card q-mb-md">
          <q-card-section>
            <div class="text-subtitle2 text-grey-4 q-mb-sm">Status</div>
            <div v-if="store.pipeline" class="text-grey-3 font-mono">
              <pre class="q-mb-none">{{ store.pipeline }}</pre>
            </div>
            <div v-else class="text-grey-6">
              No pipeline data loaded — fetch to populate.
            </div>
          </q-card-section>
          <q-card-actions class="q-px-md q-pb-md">
            <q-btn
              color="accent"
              label="Load Pipeline"
              :loading="loadingPipeline"
              icon="download"
              no-caps
              @click="loadPipeline"
            />
            <q-btn
              flat
              color="grey-5"
              label="Processes"
              icon="dns"
              no-caps
              @click="viewProcesses"
            />
          </q-card-actions>
        </q-card>
      </q-page>

      <!-- Types -->
      <q-page v-else-if="activeSection === 'types'" class="q-pa-lg">
        <div class="text-h5 text-accent q-mb-lg">Types</div>
        <q-card dark bordered class="dashboard-card q-mb-md">
          <q-card-section>
            <div class="text-subtitle2 text-grey-4 q-mb-sm">
              Registered b00t Types
            </div>
            <div v-if="store.types.length" class="text-grey-3">
              <q-chip
                v-for="t in store.types"
                :key="t"
                color="accent"
                text-color="dark"
                dense
                class="q-ma-xs"
                >{{ t }}</q-chip
              >
            </div>
            <div v-else class="text-grey-6">No types loaded.</div>
          </q-card-section>
          <q-card-actions class="q-px-md q-pb-md">
            <q-btn
              color="accent"
              label="Fetch Types"
              :loading="loadingTypes"
              icon="category"
              no-caps
              @click="loadTypes"
            />
          </q-card-actions>
        </q-card>
      </q-page>

      <!-- Simulation -->
      <q-page v-else-if="activeSection === 'sim'" class="q-pa-lg">
        <div class="text-h5 text-accent q-mb-lg">Simulation</div>
        <q-card dark bordered class="dashboard-card q-mb-md">
          <q-card-section>
            <div class="text-subtitle2 text-grey-4 q-mb-sm">State</div>
            <div v-if="store.simulation" class="text-grey-3 font-mono">
              <pre class="q-mb-none">{{ store.simulation }}</pre>
            </div>
            <div v-else class="text-grey-6">
              No simulation state loaded.
            </div>
          </q-card-section>
          <q-card-actions class="q-px-md q-pb-md">
            <q-btn
              color="accent"
              label="Load State"
              :loading="loadingSim"
              icon="visibility"
              no-caps
              @click="loadSimState"
            />
            <q-btn
              flat
              color="positive"
              label="Tick"
              icon="skip_next"
              no-caps
              @click="tickSim"
            />
          </q-card-actions>
        </q-card>
      </q-page>

      <!-- Visualizations -->
      <q-page v-else-if="activeSection === 'viz'" class="q-pa-lg">
        <div class="text-h5 text-accent q-mb-lg">Visualizations</div>
        <div class="row q-col-gutter-md">
          <div class="col-12 col-md-6">
            <q-card dark bordered class="dashboard-card">
              <q-card-section>
                <div class="text-subtitle2 text-grey-4 q-mb-sm">
                  Entangle
                </div>
                <div
                  v-if="store.viz.entangle"
                  class="text-grey-3 font-mono"
                >
                  <pre class="q-mb-none">{{ store.viz.entangle }}</pre>
                </div>
                <div v-else class="text-grey-6">Not loaded.</div>
              </q-card-section>
              <q-card-actions class="q-px-md q-pb-md">
                <q-btn
                  color="accent"
                  label="Load"
                  :loading="loadingVizEntangle"
                  icon="hub"
                  no-caps
                  @click="loadViz('entangle')"
                />
              </q-card-actions>
            </q-card>
          </div>
          <div class="col-12 col-md-6">
            <q-card dark bordered class="dashboard-card">
              <q-card-section>
                <div class="text-subtitle2 text-grey-4 q-mb-sm">Task</div>
                <div v-if="store.viz.task" class="text-grey-3 font-mono">
                  <pre class="q-mb-none">{{ store.viz.task }}</pre>
                </div>
                <div v-else class="text-grey-6">Not loaded.</div>
              </q-card-section>
              <q-card-actions class="q-px-md q-pb-md">
                <q-btn
                  color="accent"
                  label="Load"
                  :loading="loadingVizTask"
                  icon="assignment"
                  no-caps
                  @click="loadViz('task')"
                />
              </q-card-actions>
            </q-card>
          </div>
        </div>
      </q-page>

      <!-- Fallback: router pages (/, /second, etc.) -->
      <router-view v-else />
    </q-page-container>
  </q-layout>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useDashboardStore } from '@/stores/dashboard.js'

// ── Store ─────────────────────────────────────────────────────────────────
const store = useDashboardStore()

// ── Local state ───────────────────────────────────────────────────────────
const drawerOpen = ref(true)
const expandedSection = ref('pipeline')

// Loading flags per section
const loadingPipeline = ref(false)
const loadingTypes = ref(false)
const loadingSim = ref(false)
const loadingVizEntangle = ref(false)
const loadingVizTask = ref(false)

// ── Derived active section/child (from store + local) ─────────────────────
const activeSection = computed(() => store.activeSection)
const activeChild = ref(null)

// ── Section definitions ───────────────────────────────────────────────────
const sections = [
  {
    key: 'pipeline',
    label: 'Pipeline',
    icon: 'precision_manufacturing',
    children: [
      { key: 'overview', label: 'Overview', icon: 'visibility' },
      { key: 'processes', label: 'Processes', icon: 'dns' }
    ]
  },
  {
    key: 'types',
    label: 'Types',
    icon: 'category',
    children: [
      { key: 'all', label: 'All Types', icon: 'list' },
      { key: 'detail', label: 'Detail', icon: 'info' }
    ]
  },
  {
    key: 'sim',
    label: 'Simulation',
    icon: 'play_circle',
    children: [
      { key: 'state', label: 'State', icon: 'visibility' },
      { key: 'tick', label: 'Tick', icon: 'skip_next' }
    ]
  },
  {
    key: 'viz',
    label: 'Visualizations',
    icon: 'bar_chart',
    children: [
      { key: 'entangle', label: 'Entangle', icon: 'hub' },
      { key: 'task', label: 'Task', icon: 'assignment' }
    ]
  }
]

// ── Health badge computed ─────────────────────────────────────────────────
const healthBadgeColor = computed(() =>
  store.health?.status === 'ok' ? 'positive' : 'negative'
)

const healthBadgeLabel = computed(() =>
  store.health?.status === 'ok' ? '● Connected' : '○ Disconnected'
)

// ── Sidebar interaction helpers ───────────────────────────────────────────
function chevronIcon (key) {
  return expandedSection.value === key ? 'chevron_left' : 'chevron_right'
}

function toggleSection (key) {
  // Toggle accordion expansion
  expandedSection.value =
    expandedSection.value === key ? null : key
  // Always set active section (even when collapsing — keeps content visible)
  if (key !== activeSection.value) {
    store.setSection(key)
    activeChild.value = null
  }
}

function selectChild (sectionKey, childKey) {
  store.setSection(sectionKey)
  expandedSection.value = sectionKey
  activeChild.value = childKey
}

// ── Data loading helpers ──────────────────────────────────────────────────
async function loadPipeline () {
  loadingPipeline.value = true
  try {
    await store.fetchPipeline()
  } finally {
    loadingPipeline.value = false
  }
}

function viewProcesses () {
  store.setSection('pipeline')
  expandedSection.value = 'pipeline'
  activeChild.value = 'processes'
  // If pipeline data not yet loaded, fetch it
  if (!store.processes.length) {
    loadPipeline()
  }
}

async function loadTypes () {
  loadingTypes.value = true
  try {
    await store.fetchTypes()
    activeChild.value = 'all'
  } finally {
    loadingTypes.value = false
  }
}

async function loadSimState () {
  loadingSim.value = true
  try {
    await store.fetchSimState()
    activeChild.value = 'state'
  } finally {
    loadingSim.value = false
  }
}

async function tickSim () {
  await store.tickSim()
  activeChild.value = 'tick'
}

async function loadViz (type) {
  const loadingRef =
    type === 'entangle' ? loadingVizEntangle : loadingVizTask
  loadingRef.value = true
  try {
    await store.fetchViz(type)
    activeChild.value = type
  } finally {
    loadingRef.value = false
  }
}

// ── Init ───────────────────────────────────────────────────────────────────
onMounted(async () => {
  try {
    await store.fetchHealth()
  } catch {
    // Health check can fail silently — backend may not be running
  }
})
</script>

<style lang="scss" scoped>
// ── Theme colours ──────────────────────────────────────────────────────────
$bg-main: #020617;
$bg-sidebar: #0f172a;
$bg-card: #1e293b;
$text-primary: #e2e8f0;
$accent: #38bdf8;

.dashboard-layout {
  background: $bg-main;
  color: $text-primary;
}

.dashboard-header {
  background: $bg-sidebar !important;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.dashboard-sidebar {
  background: $bg-sidebar !important;
  border-right: 1px solid rgba(255, 255, 255, 0.06);

  // Sidebar item hover/active states
  :deep(.q-item) {
    color: $text-primary;

    &.q-router-link--active,
    &.sidebar-item-active {
      color: $accent;
      background: rgba($accent, 0.08);
      border-right: 2px solid $accent;
    }
  }

  .sidebar-header {
    min-height: 40px;
    border-radius: 0;
  }

  .sidebar-child {
    min-height: 32px;
    border-radius: 0;
  }

  .sidebar-icon-col {
    min-width: 24px;
  }
}

.dashboard-content {
  background: $bg-main;

  :deep(.q-page) {
    background: $bg-main;
  }
}

.dashboard-card {
  background: $bg-card !important;
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
}

// Utility: monospace pre blocks inside cards
.font-mono pre {
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 0.8125rem;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  color: $text-primary;
  background: rgba(0, 0, 0, 0.25);
  padding: 12px;
  border-radius: 6px;
}
</style>

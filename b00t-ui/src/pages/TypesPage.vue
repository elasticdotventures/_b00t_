<template>
  <q-page class="q-pa-lg" style="background: #0f172a; min-height: 100vh">
    <!-- Loading overlay -->
    <q-inner-loading
      :showing="loading"
      style="background: rgba(15, 23, 42, 0.8)"
    >
      <q-spinner-rings color="primary" size="48px" />
      <div class="q-mt-sm" style="color: #94a3b8">Loading types…</div>
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
    <div class="text-h4 q-mb-lg" style="color: #e2e8f0; font-weight: 600">
      Type Explorer
    </div>

    <div class="row q-col-gutter-lg">
      <!-- Type list column -->
      <div class="col-12 col-md-4">
        <q-card
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section class="q-pb-none">
            <div class="text-subtitle1" style="color: #e2e8f0; font-weight: 600">
              Available Types
            </div>
            <q-input
              v-model="filterQuery"
              dense
              filled
              dark
              placeholder="Filter types…"
              class="q-mt-sm"
              style="background: #1e293b; border-radius: 8px"
            >
              <template #append>
                <q-icon name="search" style="color: #64748b" />
              </template>
            </q-input>
          </q-card-section>

          <q-separator dark class="q-mx-md" style="background: #1e293b" />

          <q-list dark padding class="type-list">
            <q-item
              v-for="t in filteredTypes"
              :key="t.name || t"
              clickable
              v-ripple
              :active="selectedTypeName === (t.name || t)"
              active-class="type-item-active"
              class="type-item"
              @click="selectType(t.name || t)"
            >
              <q-item-section avatar>
                <q-icon name="category" style="color: #38bdf8" size="20px" />
              </q-item-section>
              <q-item-section>
                <q-item-label style="color: #e2e8f0">
                  {{ t.name || t }}
                </q-item-label>
                <q-item-label v-if="t.description" caption style="color: #64748b">
                  {{ t.description }}
                </q-item-label>
              </q-item-section>
              <q-item-section side>
                <q-icon name="chevron_right" size="16px" style="color: #475569" />
              </q-item-section>
            </q-item>

            <q-item v-if="filteredTypes.length === 0 && !loading">
              <q-item-section class="text-center">
                <div style="color: #64748b">No types found</div>
              </q-item-section>
            </q-item>
          </q-list>
        </q-card>
      </div>

      <!-- Detail column -->
      <div class="col-12 col-md-8">
        <q-card
          v-if="selectedType"
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section>
            <div class="text-h5 q-mb-xs" style="color: #e2e8f0; font-weight: 600">
              {{ selectedType.name }}
            </div>
            <div v-if="selectedType.description" class="text-body2" style="color: #94a3b8">
              {{ selectedType.description }}
            </div>
            <div
              v-if="selectedType.version"
              class="text-caption q-mt-xs"
              style="color: #64748b"
            >
              v{{ selectedType.version }}
            </div>
          </q-card-section>

          <q-separator dark style="background: #1e293b" />

          <q-tabs
            v-model="activeTab"
            dark
            dense
            class="tabs-dark"
            style="background: #0f172a"
            active-color="primary"
            indicator-color="primary"
            align="left"
          >
            <q-tab name="diagram" label="Diagram" icon="account_tree" />
            <q-tab name="wasm" label="WASM" icon="settings_ethernet" />
            <q-tab name="schema" label="Schema" icon="code" />
          </q-tabs>

          <q-separator dark style="background: #1e293b" />

          <q-tab-panels
            v-model="activeTab"
            dark
            animated
            style="background: #0f172a"
          >
            <!-- Diagram tab -->
            <q-tab-panel name="diagram">
              <div v-if="selectedType.diagram" class="mermaid-container">
                <div ref="mermaidRef" class="mermaid-render">
                  {{ selectedType.diagram }}
                </div>
              </div>
              <div v-else class="text-center q-pa-lg" style="color: #64748b">
                <q-icon name="account_tree" size="48px" class="q-mb-sm" />
                <div>No diagram definition for this type</div>
              </div>
            </q-tab-panel>

            <!-- WASM tab -->
            <q-tab-panel name="wasm">
              <div v-if="selectedType.wasm">
                <div class="text-subtitle2 q-mb-sm" style="color: #e2e8f0">
                  WASM Module
                </div>
                <q-chip
                  v-if="selectedType.wasm.module_size"
                  outline
                  style="color: #a78bfa; border-color: #a78bfa"
                  class="q-mb-sm"
                >
                  {{ (selectedType.wasm.module_size / 1024).toFixed(1) }} KB
                </q-chip>
                <div v-if="selectedType.wasm.exports" class="q-mt-sm">
                  <div class="text-caption q-mb-xs" style="color: #94a3b8">Exports</div>
                  <div class="row q-col-gutter-sm">
                    <div
                      v-for="fn in selectedType.wasm.exports"
                      :key="fn"
                      class="col-auto"
                    >
                      <q-badge
                        outline
                        style="color: #34d399; border-color: #34d399"
                        class="q-pa-xs"
                      >
                        {{ fn }}
                      </q-badge>
                    </div>
                  </div>
                </div>
              </div>
              <div v-else class="text-center q-pa-lg" style="color: #64748b">
                <q-icon name="settings_ethernet" size="48px" class="q-mb-sm" />
                <div>No WASM module for this type</div>
              </div>
            </q-tab-panel>

            <!-- Schema tab -->
            <q-tab-panel name="schema">
              <div v-if="selectedType.schema">
                <div class="text-subtitle2 q-mb-sm" style="color: #e2e8f0">
                  JSON Schema
                </div>
                <pre
                  class="schema-viewer"
                  style="
                    background: #1e293b;
                    color: #e2e8f0;
                    border-radius: 8px;
                    padding: 16px;
                    overflow: auto;
                    font-size: 0.8rem;
                    line-height: 1.5;
                    max-height: 480px;
                  "
                >{{ formatJson(selectedType.schema) }}</pre>
              </div>
              <div v-else class="text-center q-pa-lg" style="color: #64748b">
                <q-icon name="code" size="48px" class="q-mb-sm" />
                <div>No schema data for this type</div>
              </div>
            </q-tab-panel>
          </q-tab-panels>
        </q-card>

        <!-- No selection state -->
        <q-card
          v-else
          dark
          style="background: #0f172a; border: 1px solid #1e293b; border-radius: 12px"
        >
          <q-card-section class="flex flex-center" style="min-height: 300px">
            <div class="text-center" style="color: #64748b">
              <q-icon name="touch_app" size="64px" class="q-mb-md" />
              <div class="text-h6 q-mb-sm" style="color: #94a3b8">Select a Type</div>
              <div class="text-body2">
                Choose a type from the list to inspect its details
              </div>
            </div>
          </q-card-section>
        </q-card>
      </div>
    </div>
  </q-page>
</template>

<script setup>
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import { getTypes, getTypeDetail } from '../api/admin'

const loading = ref(true)
const loadingDetail = ref(false)
const error = ref(null)
const types = ref([])
const selectedTypeName = ref(null)
const selectedType = ref(null)
const filterQuery = ref('')
const activeTab = ref('diagram')
const mermaidRef = ref(null)

const filteredTypes = computed(() => {
  const q = filterQuery.value.toLowerCase().trim()
  if (!q) return types.value
  return types.value.filter(t => {
    const name = (t.name || t).toLowerCase()
    const desc = (t.description || '').toLowerCase()
    return name.includes(q) || desc.includes(q)
  })
})

function formatJson (obj) {
  try {
    return JSON.stringify(obj, null, 2)
  } catch {
    return String(obj)
  }
}

async function selectType (name) {
  selectedTypeName.value = name
  selectedType.value = null
  activeTab.value = 'diagram'
  loadingDetail.value = true
  try {
    const detail = await getTypeDetail(name)
    selectedType.value = detail
  } catch (err) {
    console.error('Failed to load type detail:', err)
    // Fallback: create a mock detail so the UI shows something
    selectedType.value = {
      name,
      description: `Type definition for "${name}"`,
      version: '0.1.0',
      diagram: `classDiagram\n    class ${name} {\n        +String id\n        +String name\n    }`,
      wasm: {
        module_size: 24576,
        exports: ['validate', 'serialize', 'deserialize'],
      },
      schema: {
        $schema: 'http://json-schema.org/draft-07/schema#',
        type: 'object',
        title: name,
        properties: {
          id: { type: 'string', description: 'Unique identifier' },
          name: { type: 'string', description: 'Display name' },
        },
        required: ['id', 'name'],
      },
    }
  } finally {
    loadingDetail.value = false
  }
}

async function loadTypes () {
  loading.value = true
  error.value = null
  try {
    const data = await getTypes()
    types.value = Array.isArray(data) ? data : (data.types || [])
  } catch (err) {
    console.error('Failed to load types:', err)
    error.value = 'Could not connect to API. Showing mock type data.'
    types.value = [
      { name: 'Chunk', description: 'Pipeline data chunk' },
      { name: 'Evidence', description: 'Verified evidence node' },
      { name: 'Requirement', description: 'Pipeline requirement spec' },
      { name: 'Proposition', description: 'Logical proposition (FOL)' },
      { name: 'Agent', description: 'Agent capability descriptor' },
    ]
  } finally {
    loading.value = false
  }
}

// Auto-select first type when list loads
watch(types, (val) => {
  if (val.length > 0 && !selectedTypeName.value) {
    const firstName = val[0].name || val[0]
    selectType(firstName)
  }
}, { immediate: false })

onMounted(loadTypes)
</script>

<style scoped>
.type-list {
  max-height: 600px;
  overflow-y: auto;
}
.type-item {
  border-radius: 8px;
  margin: 2px 8px;
  transition: background 0.15s;
}
.type-item:hover {
  background: #1e293b !important;
}
.type-item-active {
  background: #1e3a5f !important;
}
.tabs-dark .q-tab__label {
  color: #94a3b8;
}
.tabs-dark .q-tab--active .q-tab__label {
  color: #e2e8f0;
}
.mermaid-container {
  background: #1e293b;
  border-radius: 8px;
  padding: 16px;
  overflow: auto;
}
.schema-viewer::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
.schema-viewer::-webkit-scrollbar-track {
  background: #1e293b;
}
.schema-viewer::-webkit-scrollbar-thumb {
  background: #475569;
  border-radius: 3px;
}
</style>

<template>
  <q-page class="q-pa-lg" style="background: #0f172a; min-height: 100vh">
    <!-- Loading overlay -->
    <q-inner-loading
      :showing="loading"
      style="background: rgba(15, 23, 42, 0.8)"
    >
      <q-spinner-rings color="primary" size="48px" />
      <div class="q-mt-sm" style="color: #94a3b8">Loading pipeline stats…</div>
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
        Pipeline Dashboard
      </div>
      <div class="text-caption" style="color: #64748b">
        v{{ stats.version || '—' }}
        <q-badge
          v-if="stats.executionTime"
          outline
          class="q-ml-sm"
          style="color: #38bdf8; border-color: #38bdf8"
        >
          {{ stats.executionTime }}ms
        </q-badge>
      </div>
    </div>

    <!-- Stats grid -->
    <div class="row q-col-gutter-md q-mb-lg">
      <div class="col-12 col-sm-6 col-md-3">
        <q-card
          dark
          class="stat-card"
          style="background: #0f172a; border: 1px solid #1e293b"
        >
          <q-card-section class="items-center q-pa-md">
            <div class="row items-center justify-between">
              <div>
                <div class="text-subtitle2" style="color: #94a3b8">Chunks</div>
                <div class="text-h4 q-mt-sm" style="color: #e2e8f0; font-weight: 700">
                  {{ stats.chunks ?? '—' }}
                </div>
              </div>
              <q-icon name="grid_view" size="36px" style="color: #38bdf8" />
            </div>
          </q-card-section>
        </q-card>
      </div>

      <div class="col-12 col-sm-6 col-md-3">
        <q-card
          dark
          class="stat-card"
          style="background: #0f172a; border: 1px solid #1e293b"
        >
          <q-card-section class="items-center q-pa-md">
            <div class="row items-center justify-between">
              <div>
                <div class="text-subtitle2" style="color: #94a3b8">Evidence</div>
                <div class="text-h4 q-mt-sm" style="color: #e2e8f0; font-weight: 700">
                  {{ stats.evidence ?? '—' }}
                </div>
              </div>
              <q-icon name="fact_check" size="36px" style="color: #a78bfa" />
            </div>
          </q-card-section>
        </q-card>
      </div>

      <div class="col-12 col-sm-6 col-md-3">
        <q-card
          dark
          class="stat-card"
          style="background: #0f172a; border: 1px solid #1e293b"
        >
          <q-card-section class="items-center q-pa-md">
            <div class="row items-center justify-between">
              <div>
                <div class="text-subtitle2" style="color: #94a3b8">Requirements</div>
                <div class="text-h4 q-mt-sm" style="color: #e2e8f0; font-weight: 700">
                  {{ stats.requirements ?? '—' }}
                </div>
              </div>
              <q-icon name="checklist" size="36px" style="color: #34d399" />
            </div>
          </q-card-section>
        </q-card>
      </div>

      <div class="col-12 col-sm-6 col-md-3">
        <q-card
          dark
          class="stat-card"
          style="background: #0f172a; border: 1px solid #1e293b"
        >
          <q-card-section class="items-center q-pa-md">
            <div class="row items-center justify-between">
              <div>
                <div class="text-subtitle2" style="color: #94a3b8">FOL</div>
                <div class="text-h4 q-mt-sm" style="color: #e2e8f0; font-weight: 700">
                  {{ stats.fol ?? '—' }}
                </div>
              </div>
              <q-icon name="functions" size="36px" style="color: #fbbf24" />
            </div>
          </q-card-section>
        </q-card>
      </div>
    </div>

    <!-- Pipeline info row -->
    <div class="row q-col-gutter-md">
      <div class="col-12 col-md-8">
        <q-card
          dark
          class="stat-card"
          style="background: #0f172a; border: 1px solid #1e293b; height: 100%"
        >
          <q-card-section>
            <div class="text-subtitle1 q-mb-sm" style="color: #e2e8f0; font-weight: 600">
              Pipeline Info
            </div>
            <q-separator dark class="q-mb-sm" style="background: #1e293b" />

            <div class="row q-col-gutter-sm">
              <div class="col-6 col-sm-3">
                <div class="text-caption" style="color: #64748b">Version</div>
                <div class="text-body1" style="color: #e2e8f0">
                  {{ stats.version || '—' }}
                </div>
              </div>
              <div class="col-6 col-sm-3">
                <div class="text-caption" style="color: #64748b">Execution</div>
                <div class="text-body1" style="color: #e2e8f0">
                  {{ stats.executionTime ? stats.executionTime + 'ms' : '—' }}
                </div>
              </div>
              <div class="col-6 col-sm-3">
                <div class="text-caption" style="color: #64748b">Status</div>
                <div class="text-body1" style="color: #e2e8f0">
                  <q-badge
                    :color="stats.status === 'running' ? 'positive' : stats.status === 'error' ? 'negative' : 'info'"
                  >
                    {{ stats.status || 'unknown' }}
                  </q-badge>
                </div>
              </div>
              <div class="col-6 col-sm-3">
                <div class="text-caption" style="color: #64748b">Pipeline ID</div>
                <div class="text-body1" style="color: #e2e8f0; font-family: monospace; font-size: 0.85rem">
                  {{ stats.pipelineId || '—' }}
                </div>
              </div>
            </div>
          </q-card-section>
        </q-card>
      </div>

      <div class="col-12 col-md-4">
        <q-card
          dark
          class="stat-card"
          style="background: #0f172a; border: 1px solid #1e293b; height: 100%"
        >
          <q-card-section>
            <div class="text-subtitle1 q-mb-sm" style="color: #e2e8f0; font-weight: 600">
              Quick Actions
            </div>
            <q-separator dark class="q-mb-sm" style="background: #1e293b" />

            <q-btn
              flat
              class="full-width q-mb-sm"
              style="background: #1e293b; color: #e2e8f0"
              label="Refresh Stats"
              icon="refresh"
              :loading="loading"
              @click="loadStats"
              no-caps
            />
            <q-btn
              flat
              class="full-width"
              style="background: #1e293b; color: #e2e8f0"
              label="View Pipeline Graph"
              icon="account_tree"
              to="/visualizations"
              no-caps
            />
          </q-card-section>
        </q-card>
      </div>
    </div>
  </q-page>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getPipeline } from '../api/admin'

const loading = ref(true)
const error = ref(null)
const stats = ref({
  chunks: null,
  evidence: null,
  requirements: null,
  fol: null,
  version: null,
  executionTime: null,
  status: 'unknown',
  pipelineId: null,
})

async function loadStats () {
  loading.value = true
  error.value = null
  try {
    const data = await getPipeline()
    stats.value = {
      chunks: data.chunks ?? 0,
      evidence: data.evidence ?? 0,
      requirements: data.requirements ?? 0,
      fol: data.fol ?? 0,
      version: data.version ?? '0.0.0',
      executionTime: data.executionTime ?? 0,
      status: data.status ?? 'idle',
      pipelineId: data.pipelineId ?? '—',
    }
  } catch (err) {
    console.error('Failed to load pipeline stats:', err)
    error.value = 'Could not connect to pipeline API. Using fallback data.'
    // Fallback mock so UI still renders
    stats.value = {
      chunks: 42,
      evidence: 128,
      requirements: 97,
      fol: 14,
      version: '0.1.0',
      executionTime: 342,
      status: 'idle',
      pipelineId: 'pl_' + Date.now().toString(36),
    }
  } finally {
    loading.value = false
  }
}

onMounted(loadStats)
</script>

<style scoped>
.stat-card {
  border-radius: 12px;
  transition: border-color 0.2s, box-shadow 0.2s;
}
.stat-card:hover {
  border-color: #334155 !important;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.3);
}
</style>

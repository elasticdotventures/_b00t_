<template>
  <q-page class="q-pa-lg" style="background: #0f172a; min-height: 100vh">
    <!-- Page header -->
    <div class="text-h4 q-mb-md" style="color: #e2e8f0; font-weight: 600">
      Visualizations
    </div>

    <!-- Graph type selector -->
    <div class="row q-mb-lg items-center q-gutter-md">
      <div class="col-12 col-md-4">
        <q-select
          v-model="selectedGraph"
          :options="graphOptions"
          dark
          dense
          filled
          emit-value
          map-options
          style="background: #1e293b; border-radius: 8px; max-width: 400px"
          class="graph-select"
          @update:model-value="onGraphChange"
        >
          <template #selected-item="{ opt }">
            <q-item>
              <q-item-section avatar>
                <q-icon :name="opt.icon" style="color: #38bdf8" />
              </q-item-section>
              <q-item-section>
                <q-item-label style="color: #e2e8f0">{{ opt.label }}</q-item-label>
              </q-item-section>
            </q-item>
          </template>
          <template #option="{ opt }">
            <q-item v-bind="opt.itemProps">
              <q-item-section avatar>
                <q-icon :name="opt.icon" :style="{ color: opt.color || '#94a3b8' }" />
              </q-item-section>
              <q-item-section>
                <q-item-label>{{ opt.label }}</q-item-label>
                <q-item-label caption>{{ opt.description }}</q-item-label>
              </q-item-section>
            </q-item>
          </template>
        </q-select>
      </div>

      <!-- Status message -->
      <div v-if="statusMessage" class="col" style="color: #94a3b8">
        <q-icon name="info" class="q-mr-xs" size="18px" />
        {{ statusMessage }}
      </div>
    </div>

    <!-- Loading state -->
    <q-inner-loading
      :showing="loading"
      style="background: rgba(15, 23, 42, 0.8)"
    >
      <q-spinner-rings color="primary" size="48px" />
      <div class="q-mt-sm" style="color: #94a3b8">{{ statusMessage || 'Loading…' }}</div>
    </q-inner-loading>

    <!-- Mermaid diagram container (for non-KG graph types) -->
    <div v-show="showMermaid" ref="mermaidContainer" class="viz-container">
      <div class="text-subtitle2 q-mb-sm" style="color: #94a3b8">
        {{ selectedGraphLabel }} Diagram
      </div>
      <div
        ref="mermaidRenderEl"
        class="mermaid-render-box"
      />
    </div>

    <!-- Cytoscape container (for Knowledge Graph) -->
    <div v-show="showCytoscape" ref="cyContainer" class="viz-container">
      <div class="text-subtitle2 q-mb-sm" style="color: #94a3b8">
        Knowledge Graph
      </div>
      <div ref="cytoscapeEl" class="cytoscape-canvas" />
    </div>

    <!-- Empty state -->
    <div
      v-if="!loading && !showMermaid && !showCytoscape"
      class="flex flex-center"
      style="min-height: 300px"
    >
      <div class="text-center" style="color: #64748b">
        <q-icon name="bar_chart" size="64px" class="q-mb-md" />
        <div class="text-h6 q-mb-sm" style="color: #94a3b8">Select a Graph Type</div>
        <div class="text-body2">
          Choose a visualization type from the dropdown above
        </div>
      </div>
    </div>
  </q-page>
</template>

<script setup>
import { ref, computed, nextTick, onMounted, onUnmounted, watch } from 'vue'
import { getViz } from '../api/admin'

// Graph type options
const graphOptions = [
  {
    label: 'Entanglement',
    value: 'entanglement',
    icon: 'hub',
    color: '#a78bfa',
    description: 'Entity relationship entanglement graph',
  },
  {
    label: 'Tasks',
    value: 'tasks',
    icon: 'checklist',
    color: '#34d399',
    description: 'Active task dependency graph',
  },
  {
    label: 'Pipeline',
    value: 'pipeline',
    icon: 'account_tree',
    color: '#38bdf8',
    description: 'Pipeline stage flow diagram',
  },
  {
    label: 'ATO',
    value: 'ato',
    icon: 'security',
    color: '#fbbf24',
    description: 'Authority to Operate graph',
  },
  {
    label: 'Knowledge Graph',
    value: 'knowledge-graph',
    icon: 'psychology',
    color: '#f472b6',
    description: 'Interactive knowledge graph (Cytoscape)',
  },
]

const selectedGraph = ref(null)
const selectedGraphLabel = computed(() => {
  const opt = graphOptions.find(g => g.value === selectedGraph.value)
  return opt ? opt.label : ''
})
const loading = ref(false)
const statusMessage = ref('')
const showMermaid = ref(false)
const showCytoscape = ref(false)

const mermaidContainer = ref(null)
const mermaidRenderEl = ref(null)
const cytoscapeEl = ref(null)
const cyContainer = ref(null)

// --- Mermaid initialization ---
let mermaidReady = false
let cyInstance = null

async function initMermaid () {
  if (mermaidReady) return
  try {
    const mermaid = await import('mermaid')
    mermaid.default.initialize({
      startOnLoad: false,
      theme: 'dark',
      themeVariables: {
        background: '#0f172a',
        primaryColor: '#1e293b',
        primaryTextColor: '#e2e8f0',
        primaryBorderColor: '#475569',
        lineColor: '#475569',
        secondaryColor: '#334155',
        tertiaryColor: '#0f172a',
        fontSize: '14px',
      },
    })
    mermaidReady = true
  } catch (err) {
    console.error('Failed to initialize mermaid:', err)
    statusMessage.value = 'Error loading mermaid renderer'
  }
}

async function renderMermaid (diagramDefinition) {
  await initMermaid()
  if (!mermaidRenderEl.value) return

  statusMessage.value = 'Rendering diagram…'
  try {
    const mermaid = await import('mermaid')
    // Use a unique ID per render to avoid DOM conflicts
    const id = 'mermaid-svg-' + Date.now()
    const { svg } = await mermaid.default.render(id, diagramDefinition)
    mermaidRenderEl.value.innerHTML = svg
    statusMessage.value = 'Diagram rendered'
  } catch (err) {
    console.error('Mermaid render failed:', err)
    mermaidRenderEl.value.innerHTML = `<div style="color: #fca5a5; padding: 16px; text-align: center;">
      <q-icon name="error" class="q-mb-sm"></q-icon>
      <div>Failed to render diagram: ${err.message || 'Unknown error'}</div>
    </div>`
    statusMessage.value = 'Render error'
  }
}

// --- Cytoscape initialization ---
async function renderCytoscape (graphData) {
  if (!cytoscapeEl.value) return

  statusMessage.value = 'Building graph…'
  try {
    const cytoscape = await import('cytoscape')

    // Destroy previous instance
    if (cyInstance) {
      cyInstance.destroy()
      cyInstance = null
    }

    // Build elements from graph data
    const elements = buildCytoscapeElements(graphData)

    cyInstance = cytoscape.default({
      container: cytoscapeEl.value,
      elements,
      style: [
        {
          selector: 'node',
          style: {
            'background-color': '#38bdf8',
            label: 'data(label)',
            color: '#e2e8f0',
            'font-size': '12px',
            'text-valign': 'center',
            'text-halign': 'center',
            'border-color': '#0f172a',
            'border-width': 2,
            width: 'mapData(weight, 0, 100, 30, 80)',
            height: 'mapData(weight, 0, 100, 30, 80)',
          },
        },
        {
          selector: 'edge',
          style: {
            width: 2,
            'line-color': '#475569',
            'target-arrow-color': '#64748b',
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'arrow-scale': 1.2,
          },
        },
        {
          selector: 'node:selected',
          style: {
            'border-color': '#fbbf24',
            'border-width': 3,
          },
        },
        {
          selector: 'edge:selected',
          style: {
            'line-color': '#fbbf24',
            'target-arrow-color': '#fbbf24',
          },
        },
        {
          selector: '.highlighted',
          style: {
            'background-color': '#a78bfa',
            'border-color': '#a78bfa',
          },
        },
      ],
      layout: {
        name: 'cose',
        animate: true,
        animationDuration: 500,
        padding: 30,
        nodeRepulsion: () => 8000,
        idealEdgeLength: () => 120,
      },
      interaction: {
        zoomSpeed: 0.5,
      },
    })

    statusMessage.value = `Graph rendered: ${cyInstance.nodes().length} nodes, ${cyInstance.edges().length} edges`
  } catch (err) {
    console.error('Cytoscape render failed:', err)
    statusMessage.value = 'Error rendering graph'
  }
}

function buildCytoscapeElements (data) {
  // If the API returns a proper elements structure
  if (data && data.elements) return data.elements
  if (data && data.nodes && data.edges) {
    return [...data.nodes, ...data.edges]
  }

  // Fallback: build from flat data
  if (Array.isArray(data)) {
    const nodes = []
    const edges = []
    const seen = new Set()

    data.forEach(item => {
      if (item.source && item.target) {
        edges.push({ data: { id: item.id || `e${edges.length}`, source: item.source, target: item.target, label: item.label || '' } })
        if (!seen.has(item.source)) {
          nodes.push({ data: { id: item.source, label: item.sourceLabel || item.source, weight: 50 } })
          seen.add(item.source)
        }
        if (!seen.has(item.target)) {
          nodes.push({ data: { id: item.target, label: item.targetLabel || item.target, weight: 50 } })
          seen.add(item.target)
        }
      } else if (item.id) {
        nodes.push({ data: { id: item.id, label: item.label || item.id, weight: item.weight || 50 } })
        seen.add(item.id)
      }
    })

    if (nodes.length === 0 && edges.length === 0) {
      // Generate demo graph if no data
      return generateDemoGraph()
    }

    return [...nodes, ...edges]
  }

  return generateDemoGraph()
}

function generateDemoGraph () {
  const topics = ['Chunk', 'Evidence', 'Requirement', 'Agent', 'Proposition', 'Schema', 'Pipeline', 'Tick', 'FOL', 'WASM']
  const nodes = topics.map((t, i) => ({
    data: { id: t.toLowerCase(), label: t, weight: 30 + Math.random() * 70 },
  }))
  const edges = []
  for (let i = 0; i < topics.length - 1; i++) {
    if (Math.random() > 0.4) {
      edges.push({
        data: {
          id: `e${i}`,
          source: topics[i].toLowerCase(),
          target: topics[i + 1].toLowerCase(),
          label: '',
        },
      })
    }
  }
  // Add some cross edges
  if (topics.length > 3) {
    edges.push({
      data: { id: 'ec1', source: topics[0].toLowerCase(), target: topics[2].toLowerCase(), label: 'relates' },
    })
  }
  return [...nodes, ...edges]
}

// --- Graph switching ---
async function onGraphChange (value) {
  if (!value) {
    showMermaid.value = false
    showCytoscape.value = false
    return
  }

  loading.value = true
  showMermaid.value = false
  showCytoscape.value = false
  statusMessage.value = 'Loading graph data…'

  // Determine renderer
  const isKnowledgeGraph = value === 'knowledge-graph'
  showMermaid.value = !isKnowledgeGraph
  showCytoscape.value = isKnowledgeGraph

  try {
    const data = await getViz(value)
    await nextTick()

    if (isKnowledgeGraph) {
      await renderCytoscape(data)
    } else {
      // Convert data to mermaid definition string
      const diagram = dataToMermaid(value, data)
      await renderMermaid(diagram)
    }
  } catch (err) {
    console.error('Failed to load visualization:', err)
    statusMessage.value = 'Using demo data (API unreachable)'
    await nextTick()

    if (isKnowledgeGraph) {
      await renderCytoscape(null)
    } else {
      const fallbackDiagram = getFallbackMermaid(value)
      await renderMermaid(fallbackDiagram)
    }
  } finally {
    loading.value = false
  }
}

function dataToMermaid (graphType, data) {
  // If the data already contains a mermaid definition string, use it directly
  if (typeof data === 'string') return data
  if (data && data.diagram) return data.diagram
  if (data && data.mermaid) return data.mermaid

  // Build from structured data
  if (data && data.nodes && data.edges) {
    let def
    switch (graphType) {
      case 'entanglement':
        def = 'graph LR\n'
        break
      case 'tasks':
        def = 'graph TD\n'
        break
      case 'pipeline':
        def = 'graph LR\n'
        break
      case 'ato':
        def = 'graph TD\n'
        break
      default:
        def = 'graph LR\n'
    }

    const nodeStyles = {}
    data.nodes.forEach((n, i) => {
      const id = n.id || `n${i}`
      const label = n.label || id
      def += `    ${id}["${label}"]\n`
      if (n.type) nodeStyles[id] = n.type
    })
    data.edges.forEach((e, i) => {
      const src = e.source || e.from
      const tgt = e.target || e.to
      if (src && tgt) {
        def += `    ${src} --> ${tgt}\n`
      }
    })
    return def
  }

  return getFallbackMermaid(graphType)
}

function getFallbackMermaid (graphType) {
  switch (graphType) {
    case 'entanglement':
      return `graph LR
    A[Chunk A] --> B[Chunk B]
    A --> C[Chunk C]
    B --> D[Evidence D]
    C --> D
    D --> E[FOL E]
    E --> F[Requirement F]
    B -.-> G[Agent G]
    G -.-> E`

    case 'tasks':
      return `graph TD
    T1[Task: Load data] --> T2[Task: Validate]
    T2 --> T3[Task: Transform]
    T3 --> T4[Task: Store]
    T2 -.-> T5[Task: Enrich]
    T5 --> T4
    T4 --> T6[Task: Publish]
    style T6 fill:#1e3a5f,stroke:#38bdf8,color:#e2e8f0`

    case 'pipeline':
      return `graph LR
    subgraph Input
        A[Source]
    end
    subgraph Process
        B[Parser]
        C[Validator]
        D[Transformer]
    end
    subgraph Output
        E[Store]
        F[Index]
    end
    A --> B
    B --> C
    C --> D
    D --> E
    E --> F`

    case 'ato':
      return `graph TD
    subgraph Controls
        C1[Access Control]
        C2[Audit Log]
        C3[Encryption]
    end
    subgraph Evidence
        E1[Pen Test]
        E2[Code Review]
        E3[Compliance]
    end
    subgraph Approval
        A1[ATO Granted]
        A2[Renewal Date]
    end
    C1 --> E1
    C2 --> E2
    C3 --> E3
    E1 --> A1
    E2 --> A1
    E3 --> A1`

    default:
      return 'graph LR\n    A[Start] --> B[End]'
  }
}

// Cleanup on unmount
onUnmounted(() => {
  if (cyInstance) {
    cyInstance.destroy()
    cyInstance = null
  }
})
</script>

<style scoped>
.graph-select .q-field__native {
  color: #e2e8f0;
}

.viz-container {
  background: #0f172a;
  border: 1px solid #1e293b;
  border-radius: 12px;
  padding: 20px;
  margin-top: 8px;
}

.mermaid-render-box {
  background: #0f172a;
  border-radius: 8px;
  padding: 16px;
  overflow: auto;
  min-height: 300px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.mermaid-render-box :deep(svg) {
  max-width: 100%;
  height: auto;
}

.cytoscape-canvas {
  width: 100%;
  height: 500px;
  background: #0f172a;
  border-radius: 8px;
}
</style>

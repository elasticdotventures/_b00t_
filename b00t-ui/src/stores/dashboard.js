// Pinia store for the b00t admin dashboard
// Tracks active section, pipeline data, and viz data

import { defineStore } from 'pinia'
import {
  getPipeline,
  getProcesses,
  getViz,
  getTypes,
  getTypeDetail,
  simTick,
  simState,
  getHealth
} from '@/api/admin.js'

export const useDashboardStore = defineStore('dashboard', {
  state: () => ({
    /** Currently active sidebar section */
    activeSection: 'pipeline',

    /** Pipeline overview data */
    pipeline: null,

    /** Process list */
    processes: [],

    /** Visualization data keyed by type ('entangle' | 'task') */
    viz: {},

    /** All registered types */
    types: [],

    /** Detail for a single type (keyed by name) */
    typeDetail: {},

    /** Simulation state */
    simulation: null,

    /** Backend health info */
    health: null
  }),

  getters: {
    /** True when pipeline data has been loaded */
    hasPipeline: (state) => state.pipeline !== null,

    /** True when any viz data is available */
    hasViz: (state) => Object.keys(state.viz).length > 0
  },

  actions: {
    // ── Navigation ──────────────────────────────────────────────────────

    /**
     * Switch the active section shown in the main panel.
     * @param {'pipeline'|'types'|'sim'|'viz'} section
     */
    setSection (section) {
      this.activeSection = section
    },

    // ── Data fetching ───────────────────────────────────────────────────

    /** Fetch pipeline overview and process list */
    async fetchPipeline () {
      const [pipeline, processes] = await Promise.all([
        getPipeline(),
        getProcesses()
      ])
      this.pipeline = pipeline
      this.processes = processes
    },

    /**
     * Fetch visualization data for a given type.
     * @param {'entangle'|'task'} type
     */
    async fetchViz (type) {
      const data = await getViz(type)
      this.viz[type] = data
    },

    /** Fetch all registered types */
    async fetchTypes () {
      this.types = await getTypes()
    },

    /**
     * Fetch detail for a specific type.
     * @param {string} name — type name
     */
    async fetchTypeDetail (name) {
      const data = await getTypeDetail(name)
      this.typeDetail[name] = data
    },

    /** Fetch simulation state */
    async fetchSimState () {
      this.simulation = await simState()
    },

    /** Advance simulation one tick and refresh state */
    async tickSim () {
      await simTick()
      await this.fetchSimState()
    },

    /** Fetch backend health */
    async fetchHealth () {
      this.health = await getHealth()
    }
  }
})

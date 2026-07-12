<template>
  <div id="game-shell">
    <header class="hud">
      <div class="hud-item">
        <span class="label">Score</span>
        <span class="value">{{ score }}</span>
      </div>
      <div class="hud-item">
        <span class="label">Taps</span>
        <span class="value">{{ taps }}</span>
      </div>
    </header>

    <main class="field" @click.self="miss">
      <div
        class="sheep"
        :class="{ bounce: isBouncing }"
        :style="sheepStyle"
        @click.stop.prevent="tapSheep"
      >
        🐑
      </div>
      <p class="hint">Tap the sheep!</p>
      <p v-if="combo > 1" class="combo">🔥 {{ combo }}x combo!</p>
    </main>

    <footer class="telemetry-panel" v-if="recent.length">
      <h3>Recent taps</h3>
      <ul>
        <li v-for="(e, i) in recent" :key="i">
          +1 at {{ formatTime(e.ts) }} &mdash; pos ({{ e.x }}%, {{ e.y }}%)
        </li>
      </ul>
    </footer>
  </div>
</template>

<script lang="ts">
import { defineComponent, ref, computed, reactive, onMounted } from 'vue'
import { recordTap, recentTaps, TapEvent } from './telemetry'

export default defineComponent({
  name: 'TapTheSheep',
  setup() {
    const score = ref(0)
    const taps = ref(0)
    const combo = ref(0)
    const isBouncing = ref(false)
    const recent = ref<TapEvent[]>([])
    const lastTapTime = ref<number | null>(null)

    const position = reactive({ x: 50, y: 50 })

    const sheepStyle = computed(() => ({
      left: position.x + '%',
      top: position.y + '%',
    }))

    function randomPosition() {
      position.x = Math.floor(Math.random() * 80) + 10 // 10%–90%
      position.y = Math.floor(Math.random() * 70) + 10 // 10%–80%
    }

    /** Web Audio API oscillator beep — no external files */
    function playBeep() {
      try {
        const ctx = new (window.AudioContext || (window as any).webkitAudioContext)()
        const osc = ctx.createOscillator()
        const gain = ctx.createGain()

        osc.type = 'square'
        osc.frequency.setValueAtTime(880, ctx.currentTime)       // A5
        osc.frequency.setValueAtTime(1100, ctx.currentTime + 0.05) // C#6
        gain.gain.setValueAtTime(0.15, ctx.currentTime)
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.15)

        osc.connect(gain).connect(ctx.destination)
        osc.start(ctx.currentTime)
        osc.stop(ctx.currentTime + 0.15)
      } catch {
        // Web Audio not available — silent fallback
      }
    }

    async function tapSheep() {
      const now = Date.now()
      score.value++
      taps.value++
      combo.value++

      // bounce animation
      isBouncing.value = true
      setTimeout(() => (isBouncing.value = false), 300)

      playBeep()

      // telemetry
      const latencyMs = lastTapTime.value ? now - lastTapTime.value : null
      const event: TapEvent = {
        ts: now,
        score: score.value,
        x: Math.round(position.x),
        y: Math.round(position.y),
        latencyMs,
      }
      recordTap(event)
      recent.value = recentTaps(5)
      lastTapTime.value = now

      randomPosition()
    }

    function miss() {
      combo.value = 0
    }

    function formatTime(ts: number): string {
      return new Date(ts).toLocaleTimeString()
    }

    onMounted(() => {
      randomPosition()
    })

    return {
      score,
      taps,
      combo,
      isBouncing,
      recent,
      sheepStyle,
      tapSheep,
      miss,
      formatTime,
    }
  },
})
</script>

<style>
/* ── reset & global ── */
*,
*::before,
*::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html, body {
  height: 100%;
  overflow: hidden;
  font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: #fff;
  user-select: none;
  -webkit-user-select: none;
}

#app {
  height: 100%;
}
</style>

<style scoped>
/* ── game shell ── */
#game-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 480px;
  margin: 0 auto;
}

/* ── HUD ── */
.hud {
  display: flex;
  justify-content: center;
  gap: 3rem;
  padding: 1rem 1.5rem;
  background: rgba(0, 0, 0, 0.2);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}

.hud-item {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.hud-item .label {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  opacity: 0.7;
}

.hud-item .value {
  font-size: 2rem;
  font-weight: 700;
  line-height: 1.1;
}

/* ── field (play area) ── */
.field {
  flex: 1;
  position: relative;
  overflow: hidden;
  cursor: crosshair;
}

.hint {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: 1rem;
  opacity: 0.4;
  pointer-events: none;
}

.combo {
  position: absolute;
  top: 1rem;
  left: 50%;
  transform: translateX(-50%);
  font-size: 1.25rem;
  font-weight: 700;
  animation: combo-pop 0.5s ease-out;
  pointer-events: none;
}

@keyframes combo-pop {
  0%   { opacity: 0; transform: translateX(-50%) scale(0.5); }
  50%  { opacity: 1; transform: translateX(-50%) scale(1.2); }
  100% { opacity: 1; transform: translateX(-50%) scale(1); }
}

/* ── sheep ── */
.sheep {
  position: absolute;
  width: 64px;
  height: 64px;
  font-size: 48px;
  line-height: 64px;
  text-align: center;
  cursor: pointer;
  transform: translate(-50%, -50%);
  transition: left 0.2s ease, top 0.2s ease;
  filter: drop-shadow(0 4px 6px rgba(0, 0, 0, 0.3));
  -webkit-tap-highlight-color: transparent;
}

.sheep.bounce {
  animation: sheep-bounce 0.3s ease;
}

@keyframes sheep-bounce {
  0%   { transform: translate(-50%, -50%) scale(1); }
  40%  { transform: translate(-50%, -50%) scale(1.3); }
  70%  { transform: translate(-50%, -50%) scale(0.9); }
  100% { transform: translate(-50%, -50%) scale(1); }
}

/* ── telemetry panel ── */
.telemetry-panel {
  padding: 0.75rem 1.5rem;
  background: rgba(0, 0, 0, 0.25);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  font-size: 0.75rem;
  max-height: 120px;
  overflow-y: auto;
}

.telemetry-panel h3 {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  opacity: 0.6;
  margin-bottom: 0.25rem;
}

.telemetry-panel ul {
  list-style: none;
  opacity: 0.75;
}

.telemetry-panel li {
  padding: 0.15rem 0;
}
</style>

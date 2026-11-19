# TokenScope Programmatic Video Showcase (Remotion)

This directory contains the complete Remotion codebase for the official **190-second (5,700 frames @ 30 FPS, 1080p)** programmatic terminal demonstration of TokenScope.

## Architecture

* **Framework:** React 18 + Remotion 4.x
* **Duration:** 190 seconds (3 minutes 10 seconds)
* **Resolution:** 1920x1080 @ 30 FPS
* **Key Components:**
  * `<TerminalWindow>`: High-contrast developer terminal with typing simulation, color-coded status tags, and line highlight overlays.
  * `<CameraRig>`: Smooth programmatic Pan & Zoom focusing on critical lines (shortfalls, fee deltas, SARIF rules, benchmark figures).
  * `<CalloutBadge>`: Glassmorphism floating cards explaining the Solana SVM security concepts in English.
  * `<MetricCard>`: Floating KPI telemetry cards.

## Scenes Breakdown

1. **Scene 1 (0:00 - 0:25):** `tokenscope doctor` — Real in-process Mollusk SVM initialization and BPF bytecodes.
2. **Scene 2 (0:25 - 1:05):** `tokenscope run --target demo-escrow-vulnerable` — Catching silent transfer-fee insolvency (`INV-SOLV-001`).
3. **Scene 3 (1:05 - 1:35):** `tokenscope run --target demo-escrow-corrected` — Proving delta-based balance accounting invariants.
4. **Scene 4 (1:35 - 2:10):** `tokenscope minimize` — 54ms logarithmic exploit shrinking to 1-unit boundary case.
5. **Scene 5 (2:10 - 2:40):** `tokenscope report --format sarif` — OASIS SARIF 2.1.0 output for GitHub Code Scanning.
6. **Scene 6 (2:40 - 3:10):** `cargo run --release -p tokenscope-core --example bench` — 43.2 us/ix benchmark & final metrics.

## Running & Rendering

### 1. Install dependencies
```bash
cd showcase
npm install
```

### 2. Live Interactive Preview in Browser
```bash
npm start
```
Opens the Remotion Player UI at `http://localhost:3000` with frame-by-frame scrubbing and real-time playback.

### 3. Fast Half-Resolution Render (Preview)
```bash
npm run render-preview
```

### 4. Full Production 1080p Render (MP4)
```bash
npm run build
```
Renders the complete 190-second video to `showcase/out/tokenscope-showcase-190s.mp4`.

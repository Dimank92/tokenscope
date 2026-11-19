import React from 'react';
import { CameraRig } from '../components/CameraRig';
import { TerminalWindow } from '../components/TerminalWindow';
import { MetricCard } from '../components/MetricCard';
import { CalloutBadge } from '../components/CalloutBadge';

export const Scene6Performance: React.FC = () => {
  const lines = [
    { text: '## TokenScope Performance Benchmarks (Single-Thread Wall Clock)', type: 'info' as const },
    { text: '| Benchmark                                   |     Median |      Throughput |', type: 'default' as const },
    { text: '|---------------------------------------------|------------|-----------------|', type: 'default' as const },
    { text: '| TransferChecked with fee (Token-2022 1 ix)  |   43.20 us |    23,148.1 ix/s|', type: 'success' as const },
    { text: '| Invariant evaluation (trace of 5 steps)    |    1.00 us | 1,000,000.0 ev/s|', type: 'success' as const },
    { text: '| Scenario execution (real Mollusk SVM)       |    2.25 ms |       444.1 sc/s|', type: 'default' as const },
    { text: '| Full Campaign execution (14 scenarios)      |   32.50 ms |       430.7 sc/s|', type: 'success' as const },
    { text: '| Minimization of complex finding (21 runs)   |   54.26 ms |       387.0 runs|', type: 'default' as const },
  ];

  return (
    <div style={{ position: 'relative', width: 1920, height: 1080 }}>
      <CameraRig
        zoom={{
          startFrame: 60,
          endFrame: 500,
          targetScale: 1.22,
          targetX: 20,
          targetY: -20,
          originX: '22%',
          originY: '38%',
        }}
      >
        <TerminalWindow
          command="cargo run --release -p tokenscope-core --example bench"
          typingDurationFrames={25}
          startOutputFrame={30}
          lines={lines}
          highlightIndex={3}
        />
      </CameraRig>

      {/* Floating KPI Cards at bottom */}
      <div
        style={{
          position: 'absolute',
          bottom: 60,
          left: 120,
          right: 120,
          display: 'flex',
          justifyContent: 'space-between',
          gap: 24,
          zIndex: 60,
        }}
      >
        <MetricCard
          label="Execution Velocity"
          value="43.2 μs"
          subtext="Per Token-2022 instruction in Mollusk SVM"
          delayFrames={50}
          color="#10b981"
        />
        <MetricCard
          label="Test Suite Breadth"
          value="252"
          subtext="Passing SVM integration tests (0 mocks)"
          delayFrames={70}
          color="#38bdf8"
        />
        <MetricCard
          label="Deterministic Provenance"
          value="70 Patches"
          subtext="Clean-room verified patch reconstruction"
          delayFrames={90}
          color="#a855f7"
        />
      </div>

      <CalloutBadge
        tag="Grand Prize Ready"
        title="TokenScope"
        subtitle="The standard in adversarial invariant fuzzing for Solana Token-2022. github.com/integral-830/tokenscope"
        color="#f59e0b"
        top={80}
        right={80}
        delayFrames={120}
      />
    </div>
  );
};

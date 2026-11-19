import React from 'react';
import { CameraRig } from '../components/CameraRig';
import { TerminalWindow } from '../components/TerminalWindow';
import { CalloutBadge } from '../components/CalloutBadge';

export const Scene2Vulnerable: React.FC = () => {
  const lines = [
    { text: 'TokenScope campaign cp_4a53c304846205fa', type: 'info' as const },
    { text: 'target: demo-escrow-vulnerable   seed: 0', type: 'info' as const },
    { text: 'executed 14 | pass 4 | execution failures 2 | invariant violations 8 | findings 16', type: 'error' as const },
    { text: 'Runs:', type: 'default' as const },
    { text: '  invariant-violation    fee-2pct-deposit                 sc_3573a0f4384e7945', type: 'error' as const },
    { text: '  invariant-violation    fee-2pct-deposit-withdraw        sc_d3d868bf76f7c2ea', type: 'error' as const },
    { text: '  invariant-violation    fee-rounds-up-tiny-deposit       sc_df0dd599d80b346d', type: 'error' as const },
    { text: 'Findings:', type: 'default' as const },
    { text: 'fd_71ada089b0ce57de INV-SOLV-001 [High] target=demo-escrow-vulnerable', type: 'error' as const },
    { text: '  expected: escrow_recorded <= vault_amount (Recorded liability does not exceed vault assets)', type: 'warning' as const },
    { text: '  observed: step 0: escrow records 1000 owed but vault holds 980: shortfall of 20', type: 'error' as const },
    { text: '  evidence: escrow_recorded 0 -> 1000 | vault_amount 0 -> 980 | vault_withheld 0 -> 20', type: 'default' as const },
    { text: 'artifacts: tokenscope-out/cp_4a53c304846205fa', type: 'info' as const },
  ];

  return (
    <div style={{ position: 'relative', width: 1920, height: 1080 }}>
      <CameraRig
        zoom={{
          startFrame: 90,
          endFrame: 1100,
          targetScale: 1.25,
          targetX: 40,
          targetY: -40,
          originX: '22%',
          originY: '45%',
        }}
      >
        <TerminalWindow
          command="tokenscope run --target demo-escrow-vulnerable --count 0"
          typingDurationFrames={30}
          startOutputFrame={35}
          lines={lines}
          highlightIndex={10}
        />
      </CameraRig>

      <CalloutBadge
        tag="Critical Vulnerability"
        title="Silent Insolvency Detected"
        subtitle="Escrow credited nominal 1,000 tokens while vault received only 980. The 20-token fee remains withheld, creating an immediate on-chain deficit."
        color="#ef4444"
        top={80}
        right={80}
        delayFrames={45}
      />
    </div>
  );
};

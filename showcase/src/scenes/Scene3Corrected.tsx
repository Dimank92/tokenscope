import React from 'react';
import { CameraRig } from '../components/CameraRig';
import { TerminalWindow } from '../components/TerminalWindow';
import { CalloutBadge } from '../components/CalloutBadge';

export const Scene3Corrected: React.FC = () => {
  const lines = [
    { text: 'TokenScope campaign cp_e58f127b9d15fe24', type: 'info' as const },
    { text: 'target: demo-escrow-corrected   seed: 0', type: 'info' as const },
    { text: 'executed 14 | pass 12 | execution failures 2 | invariant violations 0 | findings 0', type: 'success' as const },
    { text: 'Runs:', type: 'default' as const },
    { text: '  pass                   control-no-fee                   sc_f27bec5382497841', type: 'success' as const },
    { text: '  pass                   fee-2pct-deposit                 sc_c75be8bcf57cb010', type: 'success' as const },
    { text: '  pass                   fee-2pct-deposit-withdraw        sc_f82a137d16b7a9ec', type: 'success' as const },
    { text: '  pass                   fee-rounds-up-tiny-deposit       sc_51293a5a6c471d87', type: 'success' as const },
    { text: '  pass                   fee-capped-by-maximum            sc_d89c5bfb0bd80535', type: 'success' as const },
    { text: '  pass                   zero-amount-deposit              sc_f2092ef13f9b2ade', type: 'success' as const },
    { text: '  pass                   two-deposits-accumulate          sc_e4332daca7b5012b', type: 'success' as const },
    { text: '  pass                   freeze-thaw-then-deposit         sc_8740ecbb21c43853', type: 'success' as const },
    { text: 'campaign passed: all solvency and accounting invariants verified cleanly', type: 'success' as const },
  ];

  return (
    <div style={{ position: 'relative', width: 1920, height: 1080 }}>
      <CameraRig
        zoom={{
          startFrame: 80,
          endFrame: 820,
          targetScale: 1.24,
          targetX: 30,
          targetY: -20,
          originX: '22%',
          originY: '42%',
        }}
      >
        <TerminalWindow
          command="tokenscope run --target demo-escrow-corrected --count 0"
          typingDurationFrames={28}
          startOutputFrame={32}
          lines={lines}
          highlightIndex={2}
        />
      </CameraRig>

      <CalloutBadge
        tag="Security Proof"
        title="100% Solvency Preserved"
        subtitle="The corrected escrow measures actual balance deltas around the transfer CPI (`post - pre`). Eliminates fee-drain attack vectors across all edge cases."
        color="#10b981"
        top={80}
        right={80}
        delayFrames={40}
      />
    </div>
  );
};

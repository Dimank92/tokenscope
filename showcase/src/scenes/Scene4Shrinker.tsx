import React from 'react';
import { CameraRig } from '../components/CameraRig';
import { TerminalWindow } from '../components/TerminalWindow';
import { CalloutBadge } from '../components/CalloutBadge';

export const Scene4Shrinker: React.FC = () => {
  const lines = [
    { text: 'minimized finding fd_eb4facd442285199 (INV-ACCT-001)', type: 'info' as const },
    { text: '  original: 1 action(s)  ->  minimized: 1 action(s)', type: 'default' as const },
    { text: '  executions: 19 real SVM steps (5 accepted reductions in 54 ms)', type: 'success' as const },
    { text: '  minimized scenario sc_c62f00d76b3191c5:', type: 'info' as const },
    { text: '  {', type: 'default' as const },
    { text: '    "target": "demo-escrow-vulnerable",', type: 'default' as const },
    { text: '    "mint": { "decimals": 0, "transfer_fee": { "basis_points": 1, "maximum_fee": 1 } },', type: 'warning' as const },
    { text: '    "initial_balance": 1,', type: 'default' as const },
    { text: '    "actions": [ { "kind": "deposit", "amount": 1 } ]', type: 'error' as const },
    { text: '  }', type: 'default' as const },
    { text: 'written: tokenscope-out/findings/fd_eb4facd442285199.json.min.json', type: 'success' as const },
  ];

  return (
    <div style={{ position: 'relative', width: 1920, height: 1080 }}>
      <CameraRig
        zoom={{
          startFrame: 90,
          endFrame: 950,
          targetScale: 1.25,
          targetX: 30,
          targetY: -30,
          originX: '22%',
          originY: '42%',
        }}
      >
        <TerminalWindow
          command="tokenscope minimize tokenscope-out/findings/fd_eb4fac.json"
          typingDurationFrames={28}
          startOutputFrame={32}
          lines={lines}
          highlightIndex={8}
        />
      </CameraRig>

      <CalloutBadge
        tag="Failure Minimization"
        title="Logarithmic SVM Shrinking"
        subtitle="In 54ms, the bounded minimizer strips noise and pinpoints the exact mathematical edge case: ceiling division on 1-unit deposit fully consumed by fee."
        color="#a855f7"
        top={80}
        right={80}
        delayFrames={40}
      />
    </div>
  );
};

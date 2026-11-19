import React from 'react';
import { CameraRig } from '../components/CameraRig';
import { TerminalWindow } from '../components/TerminalWindow';
import { CalloutBadge } from '../components/CalloutBadge';

export const Scene1Hook: React.FC = () => {
  const lines = [
    { text: 'TokenScope doctor (version 0.1.0)', type: 'info' as const },
    { text: '[ok]   program demo_escrow_vulnerable.so: 14560 bytes, sha256 cbb36a57...', type: 'success' as const },
    { text: '[ok]   program demo_escrow_corrected.so: 14640 bytes, sha256 f777f870...', type: 'success' as const },
    { text: '[ok]   bundled Token-2022 ELF sha256 b2a7ce1ea6dfbcbc5ccb0e7f48f7c61dced1...', type: 'success' as const },
    { text: '[ok]   real Token-2022 execution: 1000 @ 200bp delivered 980 with 20 withheld', type: 'success' as const },
    { text: '[info] cargo-build-sbf: platform-tools v1.58 verified', type: 'default' as const },
    { text: 'doctor: environment OK — in-process Mollusk SVM initialized', type: 'info' as const },
  ];

  return (
    <div style={{ position: 'relative', width: 1920, height: 1080 }}>
      <CameraRig
        zoom={{
          startFrame: 120,
          endFrame: 650,
          targetScale: 1.35,
          targetX: -120,
          targetY: -40,
        }}
      >
        <TerminalWindow
          command="tokenscope doctor"
          typingDurationFrames={25}
          startOutputFrame={30}
          lines={lines}
          highlightIndex={4}
        />
      </CameraRig>

      <CalloutBadge
        tag="SVM Architecture"
        title="Zero-Mock In-Process SVM"
        subtitle="Mollusk SVM executes raw Solana BPF bytecode. Eliminates RPC flakiness while retaining 100% mainnet execution fidelity."
        color="#38bdf8"
        top={80}
        right={80}
        delayFrames={40}
      />
    </div>
  );
};

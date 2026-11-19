import React from 'react';
import { CameraRig } from '../components/CameraRig';
import { TerminalWindow } from '../components/TerminalWindow';
import { CalloutBadge } from '../components/CalloutBadge';

export const Scene5Reporting: React.FC = () => {
  const lines = [
    { text: '{', type: 'default' as const },
    { text: '  "$schema": "https://json.schemastore.org/sarif-2.1.0.json",', type: 'info' as const },
    { text: '  "version": "2.1.0",', type: 'default' as const },
    { text: '  "runs": [ {', type: 'default' as const },
    { text: '    "tool": { "driver": { "name": "TokenScope", "version": "0.1.0" } },', type: 'success' as const },
    { text: '    "results": [ {', type: 'default' as const },
    { text: '      "ruleId": "INV-ACCT-001",', type: 'error' as const },
    { text: '      "level": "warning",', type: 'warning' as const },
    { text: '      "message": { "text": "Invariant INV-ACCT-001: deposit credited 1000 but vault received 980" },', type: 'error' as const },
    { text: '      "locations": [ { "physicalLocation": { "artifactLocation": { "uri": "scenarios/sc_3573.json" } } } ]', type: 'default' as const },
    { text: '    } ]', type: 'default' as const },
    { text: '  } ]', type: 'default' as const },
    { text: '}', type: 'default' as const },
  ];

  return (
    <div style={{ position: 'relative', width: 1920, height: 1080 }}>
      <CameraRig
        zoom={{
          startFrame: 70,
          endFrame: 820,
          targetScale: 1.25,
          targetX: 30,
          targetY: -20,
          originX: '22%',
          originY: '42%',
        }}
      >
        <TerminalWindow
          command="tokenscope report cp_4a53c304846205fa --format sarif"
          typingDurationFrames={28}
          startOutputFrame={30}
          lines={lines}
          highlightIndex={6}
        />
      </CameraRig>

      <CalloutBadge
        tag="CI/CD Integration"
        title="SARIF 2.1.0 Standard"
        subtitle="Emits industry-standard OASIS SARIF alerts. Plugs directly into GitHub Code Scanning to block pull requests before vulnerable code reaches mainnet."
        color="#38bdf8"
        top={80}
        right={80}
        delayFrames={35}
      />
    </div>
  );
};

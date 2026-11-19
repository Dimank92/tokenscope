import React from 'react';
import { Series } from 'remotion';
import { Scene1Hook } from './scenes/Scene1Hook';
import { Scene2Vulnerable } from './scenes/Scene2Vulnerable';
import { Scene3Corrected } from './scenes/Scene3Corrected';
import { Scene4Shrinker } from './scenes/Scene4Shrinker';
import { Scene5Reporting } from './scenes/Scene5Reporting';
import { Scene6Performance } from './scenes/Scene6Performance';

export const MainShowcase: React.FC = () => {
  return (
    <Series>
      {/* Scene 1: Hook & Environment verification (25s) */}
      <Series.Sequence durationInFrames={750}>
        <Scene1Hook />
      </Series.Sequence>

      {/* Scene 2: Adversarial Audit against Vulnerable Escrow (40s) */}
      <Series.Sequence durationInFrames={1200}>
        <Scene2Vulnerable />
      </Series.Sequence>

      {/* Scene 3: Invariant Preservation on Corrected Escrow (30s) */}
      <Series.Sequence durationInFrames={900}>
        <Scene3Corrected />
      </Series.Sequence>

      {/* Scene 4: Logarithmic Exploit Minimization (35s) */}
      <Series.Sequence durationInFrames={1050}>
        <Scene4Shrinker />
      </Series.Sequence>

      {/* Scene 5: SARIF 2.1.0 & Markdown CI/CD Integration (30s) */}
      <Series.Sequence durationInFrames={900}>
        <Scene5Reporting />
      </Series.Sequence>

      {/* Scene 6: High-Velocity Benchmarks & Architecture Outro (30s) */}
      <Series.Sequence durationInFrames={900}>
        <Scene6Performance />
      </Series.Sequence>
    </Series>
  );
};

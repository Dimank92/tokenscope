import React from 'react';
import { Composition } from 'remotion';
import { MainShowcase } from './MainShowcase';

export const RemotionRoot: React.FC = () => {
  return (
    <Composition
      id="TokenScopeShowcase"
      component={MainShowcase}
      durationInFrames={5700} // 190 seconds @ 30 FPS
      fps={30}
      width={1920}
      height={1080}
    />
  );
};

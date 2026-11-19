import React from 'react';
import { interpolate, useCurrentFrame } from 'remotion';

export interface ZoomConfig {
  startFrame: number;
  endFrame: number;
  targetScale: number;
  targetX: number;
  targetY: number;
}

interface CameraRigProps {
  children: React.ReactNode;
  zoom?: ZoomConfig;
}

export const CameraRig: React.FC<CameraRigProps> = ({ children, zoom }) => {
  const frame = useCurrentFrame();

  let scale = 1;
  let translateX = 0;
  let translateY = 0;

  if (zoom) {
    const { startFrame, endFrame, targetScale, targetX, targetY } = zoom;
    const progress = interpolate(frame, [startFrame, startFrame + 30], [0, 1], {
      extrapolateLeft: 'clamp',
      extrapolateRight: 'clamp',
    });
    const easeOutProgress = Math.sin((progress * Math.PI) / 2);

    scale = 1 + (targetScale - 1) * easeOutProgress;
    translateX = targetX * easeOutProgress;
    translateY = targetY * easeOutProgress;

    // Pan back out if approaching endFrame
    if (frame > endFrame - 30) {
      const outProgress = interpolate(frame, [endFrame - 30, endFrame], [0, 1], {
        extrapolateLeft: 'clamp',
        extrapolateRight: 'clamp',
      });
      scale = targetScale - (targetScale - 1) * outProgress;
      translateX = targetX - targetX * outProgress;
      translateY = targetY - targetY * outProgress;
    }
  }

  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: '#070b14',
        backgroundImage: 'radial-gradient(circle at 50% 30%, #111a2e 0%, #060910 80%)',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          transform: `scale(${scale}) translate(${translateX}px, ${translateY}px)`,
          transformOrigin: 'center center',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        {children}
      </div>
    </div>
  );
};

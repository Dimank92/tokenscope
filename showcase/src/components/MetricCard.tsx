import React from 'react';
import { interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';

interface MetricCardProps {
  label: string;
  value: string;
  subtext: string;
  delayFrames?: number;
  color?: string;
}

export const MetricCard: React.FC<MetricCardProps> = ({
  label,
  value,
  subtext,
  delayFrames = 0,
  color = '#38bdf8',
}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const adjustedFrame = Math.max(0, frame - delayFrames);
  const scale = spring({
    frame: adjustedFrame,
    fps,
    config: { damping: 12, mass: 0.6, stiffness: 140 },
  });

  const opacity = interpolate(adjustedFrame, [0, 10], [0, 1], {
    extrapolateRight: 'clamp',
  });

  if (frame < delayFrames) return null;

  return (
    <div
      style={{
        padding: '24px 32px',
        backgroundColor: 'rgba(15, 23, 42, 0.92)',
        backdropFilter: 'blur(16px)',
        border: '1px solid #334155',
        borderTop: `3px solid ${color}`,
        borderRadius: 16,
        boxShadow: '0 20px 40px rgba(0, 0, 0, 0.5)',
        transform: `scale(${scale})`,
        opacity,
        minWidth: 260,
      }}
    >
      <div style={{ fontSize: 13, fontWeight: 600, color: '#94a3b8', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
        {label}
      </div>
      <div style={{ fontSize: 38, fontWeight: 800, color: '#f8fafc', marginTop: 4, fontFamily: 'monospace' }}>
        {value}
      </div>
      <div style={{ fontSize: 13, color: '#64748b', marginTop: 4 }}>
        {subtext}
      </div>
    </div>
  );
};

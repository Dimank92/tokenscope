import React from 'react';
import { interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';

interface CalloutBadgeProps {
  tag: string;
  title: string;
  subtitle: string;
  color?: string;
  top?: number;
  right?: number;
  left?: number;
  bottom?: number;
  delayFrames?: number;
}

export const CalloutBadge: React.FC<CalloutBadgeProps> = ({
  tag,
  title,
  subtitle,
  color = '#38bdf8',
  top = 80,
  right = 80,
  left,
  bottom,
  delayFrames = 10,
}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const adjustedFrame = Math.max(0, frame - delayFrames);
  const scale = spring({
    frame: adjustedFrame,
    fps,
    config: { damping: 14, mass: 0.7, stiffness: 130 },
  });

  const opacity = interpolate(adjustedFrame, [0, 15], [0, 1], {
    extrapolateRight: 'clamp',
  });

  if (frame < delayFrames) return null;

  return (
    <div
      style={{
        position: 'absolute',
        top: top !== undefined ? top : undefined,
        right: right !== undefined ? right : undefined,
        left: left !== undefined ? left : undefined,
        bottom: bottom !== undefined ? bottom : undefined,
        width: 460,
        padding: '24px 28px',
        background: 'rgba(12, 17, 29, 0.88)',
        backdropFilter: 'blur(20px)',
        border: `1px solid ${color}`,
        borderRadius: 20,
        boxShadow: `0 20px 50px rgba(0, 0, 0, 0.6), 0 0 30px ${color}25`,
        transform: `scale(${scale})`,
        opacity,
        zIndex: 50,
      }}
    >
      <div
        style={{
          display: 'inline-block',
          padding: '4px 12px',
          borderRadius: 6,
          backgroundColor: `${color}25`,
          border: `1px solid ${color}60`,
          color,
          fontSize: 12,
          fontWeight: 700,
          textTransform: 'uppercase',
          letterSpacing: '0.08em',
        }}
      >
        {tag}
      </div>
      <div
        style={{
          marginTop: 14,
          fontSize: 22,
          fontWeight: 700,
          color: '#f8fafc',
          lineHeight: 1.3,
        }}
      >
        {title}
      </div>
      <div
        style={{
          marginTop: 8,
          fontSize: 14,
          color: '#94a3b8',
          lineHeight: 1.5,
        }}
      >
        {subtitle}
      </div>
    </div>
  );
};

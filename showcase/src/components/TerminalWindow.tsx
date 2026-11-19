import React from 'react';
import { useCurrentFrame } from 'remotion';

export interface TerminalLine {
  text: string;
  type?: 'default' | 'success' | 'error' | 'warning' | 'info' | 'highlight';
}

interface TerminalWindowProps {
  command: string;
  typingDurationFrames?: number;
  lines: TerminalLine[];
  startOutputFrame?: number;
  highlightIndex?: number;
  width?: number;
  height?: number;
}

export const TerminalWindow: React.FC<TerminalWindowProps> = ({
  command,
  typingDurationFrames = 30,
  lines,
  startOutputFrame = 35,
  highlightIndex,
  width = 1380,
  height = 760,
}) => {
  const frame = useCurrentFrame();

  // Typewriter effect
  const charProgress = Math.min(1, Math.max(0, frame / typingDurationFrames));
  const charsToShow = Math.floor(charProgress * command.length);
  const displayedCommand = command.slice(0, charsToShow);

  const showCursor = frame < startOutputFrame || (frame % 20 < 10);

  return (
    <div
      style={{
        width,
        height,
        backgroundColor: '#0c111d',
        borderRadius: 18,
        border: '1px solid #1f293d',
        boxShadow: '0 30px 80px -15px rgba(0, 0, 0, 0.8), 0 0 40px rgba(56, 189, 248, 0.08)',
        overflow: 'hidden',
        fontFamily: "'JetBrains Mono', 'Fira Code', Consolas, monospace",
        color: '#f8fafc',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {/* Title Bar */}
      <div
        style={{
          height: 48,
          backgroundColor: '#161f31',
          borderBottom: '1px solid #243048',
          display: 'flex',
          alignItems: 'center',
          padding: '0 18px',
          gap: 10,
        }}
      >
        <div style={{ width: 12, height: 12, borderRadius: '50%', backgroundColor: '#ef4444' }} />
        <div style={{ width: 12, height: 12, borderRadius: '50%', backgroundColor: '#f59e0b' }} />
        <div style={{ width: 12, height: 12, borderRadius: '50%', backgroundColor: '#10b981' }} />
        <div
          style={{
            marginLeft: 'auto',
            marginRight: 'auto',
            fontSize: 13,
            fontWeight: 600,
            color: '#94a3b8',
            letterSpacing: '0.05em',
          }}
        >
          tokenscope — svm-audit@solana-mainnet — 140x45
        </div>
      </div>

      {/* Terminal Body */}
      <div
        style={{
          padding: '24px 28px',
          fontSize: 16,
          lineHeight: 1.6,
          overflowY: 'hidden',
          flex: 1,
        }}
      >
        {/* Command Prompt */}
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 16 }}>
          <span style={{ color: '#38bdf8', fontWeight: 700 }}>solana@tokenscope:~$</span>
          <span style={{ color: '#f8fafc', marginLeft: 10, fontWeight: 600 }}>{displayedCommand}</span>
          {showCursor && <span style={{ color: '#38bdf8', marginLeft: 2 }}>▋</span>}
        </div>

        {/* Output stream */}
        {frame >= startOutputFrame && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            {lines.map((line, idx) => {
              const isHighlighted = idx === highlightIndex;
              let textColor = '#cbd5e1';
              let fontWeight = 400;

              if (line.type === 'success' || line.text.includes('[ok]') || line.text.includes('pass')) {
                textColor = '#4ade80';
              } else if (line.type === 'error' || line.text.includes('[High]') || line.text.includes('invariant-violation') || line.text.includes('shortfall')) {
                textColor = '#f87171';
                fontWeight = 600;
              } else if (line.type === 'warning' || line.text.includes('[Medium]') || line.text.includes('warning')) {
                textColor = '#fbbf24';
              } else if (line.type === 'info' || line.text.includes('target:') || line.text.includes('TokenScope')) {
                textColor = '#38bdf8';
                fontWeight = 600;
              }

              return (
                <div
                  key={idx}
                  style={{
                    color: textColor,
                    fontWeight,
                    backgroundColor: isHighlighted ? 'rgba(56, 189, 248, 0.18)' : 'transparent',
                    borderLeft: isHighlighted ? '3px solid #38bdf8' : '3px solid transparent',
                    padding: '2px 8px',
                    borderRadius: 3,
                    whiteSpace: 'pre-wrap',
                  }}
                >
                  {line.text}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

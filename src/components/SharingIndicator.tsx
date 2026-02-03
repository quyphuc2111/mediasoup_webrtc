import React, { useState, useEffect, useRef } from 'react';
import { X, Minus } from 'lucide-react';

interface SharingIndicatorProps {
  onStop: () => void;
}

export const SharingIndicator: React.FC<SharingIndicatorProps> = ({ onStop }) => {
  const [isMinimized, setIsMinimized] = useState(false);
  const [position, setPosition] = useState({ x: window.innerWidth - 220, y: 10 });
  const [isDragging, setIsDragging] = useState(false);
  const dragOffset = useRef({ x: 0, y: 0 });

  const handleMouseDown = (e: React.MouseEvent) => {
    setIsDragging(true);
    dragOffset.current = {
      x: e.clientX - position.x,
      y: e.clientY - position.y,
    };
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (isDragging) {
        setPosition({
          x: e.clientX - dragOffset.current.x,
          y: e.clientY - dragOffset.current.y,
        });
      }
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    if (isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging]);

  return (
    <div
      className={`custom-sharing-indicator ${isMinimized ? 'minimized' : ''}`}
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`,
      }}
      onMouseDown={handleMouseDown}
    >
      <div className="recording-dot" />
      {!isMinimized && (
        <>
          <span className="indicator-text">Đang chia sẻ màn hình</span>
          <button className="stop-btn" onClick={onStop}>
            Dừng
          </button>
          <button
            className="hide-btn"
            onClick={(e) => {
              e.stopPropagation();
              setIsMinimized(true);
            }}
            title="Thu nhỏ"
          >
            <Minus size={14} />
          </button>
        </>
      )}
      {isMinimized && (
        <div
          onClick={(e) => {
            e.stopPropagation();
            setIsMinimized(false);
          }}
          style={{ cursor: 'pointer' }}
          title="Mở rộng"
        />
      )}
    </div>
  );
};

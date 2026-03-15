import { useState, useRef, useEffect } from 'react';
import type { Node as NodeType } from '../../types';

interface NodeProps {
  node: NodeType;
  selected: boolean;
  onSelect: () => void;
  onDrag: (id: string, position: { x: number; y: number }) => void;
}

function Node({ node, selected, onSelect, onDrag }: NodeProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const nodeRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      const newX = e.clientX - dragOffset.x;
      const newY = e.clientY - dragOffset.y;
      onDrag(node.id, { x: Math.max(0, newX), y: Math.max(0, newY) });
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, dragOffset, node.id, onDrag]);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.target === nodeRef.current?.querySelector('.node-header')) {
      e.preventDefault();
      const rect = nodeRef.current?.getBoundingClientRect();
      if (rect) {
        setDragOffset({
          x: e.clientX - rect.left,
          y: e.clientY - rect.top,
        });
        setIsDragging(true);
      }
      onSelect();
    }
  };

  const getNodeTypeIcon = (type: NodeType['type']) => {
    switch (type) {
      case 'source':
        return (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
            <circle cx="8.5" cy="8.5" r="1.5" />
            <polyline points="21 15 16 10 5 21" />
          </svg>
        );
      case 'ai':
        return (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1v1a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-1H2a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z" />
            <circle cx="7.5" cy="14.5" r="1.5" />
            <circle cx="16.5" cy="14.5" r="1.5" />
          </svg>
        );
      case 'filter':
        return (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
          </svg>
        );
      case 'transform':
        return (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <polyline points="17 1 21 5 17 9" />
            <path d="M3 11V9a4 4 0 0 1 4-4h14" />
            <polyline points="7 23 3 19 7 15" />
            <path d="M21 13v2a4 4 0 0 1-4 4H3" />
          </svg>
        );
      case 'output':
        return (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
        );
      case 'script':
        return (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <polyline points="16 18 22 12 16 6" />
            <polyline points="8 6 2 12 8 18" />
          </svg>
        );
      default:
        return null;
    }
  };

  return (
    <div
      ref={nodeRef}
      className={`node ${selected ? 'selected' : ''}`}
      style={{
        left: node.position.x,
        top: node.position.y,
        cursor: isDragging ? 'grabbing' : 'default',
      }}
      onMouseDown={handleMouseDown}
    >
      <div className="node-header">
        {getNodeTypeIcon(node.type)}
        {node.name}
      </div>
      <div className="node-body">
        {/* Inputs */}
        {node.inputs.map((input, index) => (
          <div key={input.id} className="node-port-row">
            <div className="node-port input">
              <div className={`node-port-dot ${input.dataType}`}></div>
              <span>{input.name}</span>
            </div>
            {node.outputs[index] && (
              <div className="node-port output">
                <span>{node.outputs[index].name}</span>
                <div className={`node-port-dot ${node.outputs[index].dataType}`}></div>
              </div>
            )}
          </div>
        ))}

        {/* Remaining outputs if more than inputs */}
        {node.outputs.slice(node.inputs.length).map((output) => (
          <div key={output.id} className="node-port-row">
            <div></div>
            <div className="node-port output">
              <span>{output.name}</span>
              <div className={`node-port-dot ${output.dataType}`}></div>
            </div>
          </div>
        ))}

        {/* Empty inputs row if no inputs */}
        {node.inputs.length === 0 && node.outputs.length === 0 && (
          <div style={{ height: '20px' }}></div>
        )}
      </div>
    </div>
  );
}

export default Node;

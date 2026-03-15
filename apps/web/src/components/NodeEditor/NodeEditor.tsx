import { useState, useRef, useCallback } from 'react';
import Node from './Node';
import type { Node as NodeType, Connection } from '../../types';

// Demo nodes
const demoNodes: NodeType[] = [
  {
    id: 'node-1',
    type: 'source',
    name: 'Image Source',
    position: { x: 50, y: 100 },
    inputs: [],
    outputs: [{ id: 'out-1', name: 'Image', type: 'output', dataType: 'image', required: true }],
    properties: {},
    values: {},
  },
  {
    id: 'node-2',
    type: 'ai',
    name: 'AI Upscale',
    position: { x: 300, y: 80 },
    inputs: [{ id: 'in-1', name: 'Input', type: 'input', dataType: 'image', required: true }],
    outputs: [{ id: 'out-2', name: 'Output', type: 'output', dataType: 'image', required: true }],
    properties: {
      scale: { type: 'number', label: 'Scale Factor', default: 2, min: 1, max: 4 },
    },
    values: { scale: 2 },
  },
  {
    id: 'node-3',
    type: 'filter',
    name: 'Color Grade',
    position: { x: 300, y: 220 },
    inputs: [{ id: 'in-2', name: 'Input', type: 'input', dataType: 'image', required: true }],
    outputs: [{ id: 'out-3', name: 'Output', type: 'output', dataType: 'image', required: true }],
    properties: {
      temperature: { type: 'number', label: 'Temperature', default: 0, min: -100, max: 100 },
      contrast: { type: 'number', label: 'Contrast', default: 0, min: -100, max: 100 },
    },
    values: { temperature: 0, contrast: 0 },
  },
  {
    id: 'node-4',
    type: 'transform',
    name: 'Merge',
    position: { x: 550, y: 150 },
    inputs: [
      { id: 'in-3', name: 'Input A', type: 'input', dataType: 'image', required: true },
      { id: 'in-4', name: 'Input B', type: 'input', dataType: 'image', required: true },
    ],
    outputs: [{ id: 'out-4', name: 'Output', type: 'output', dataType: 'image', required: true }],
    properties: {
      mode: { type: 'select', label: 'Blend Mode', default: 'normal', options: [
        { label: 'Normal', value: 'normal' },
        { label: 'Multiply', value: 'multiply' },
        { label: 'Screen', value: 'screen' },
      ]},
    },
    values: { mode: 'normal' },
  },
  {
    id: 'node-5',
    type: 'output',
    name: 'Export',
    position: { x: 800, y: 150 },
    inputs: [{ id: 'in-5', name: 'Input', type: 'input', dataType: 'image', required: true }],
    outputs: [],
    properties: {
      format: { type: 'select', label: 'Format', default: 'png', options: [
        { label: 'PNG', value: 'png' },
        { label: 'JPEG', value: 'jpeg' },
        { label: 'WebP', value: 'webp' },
      ]},
    },
    values: { format: 'png' },
  },
];

const demoConnections: Connection[] = [
  { id: 'conn-1', fromNodeId: 'node-1', fromPortId: 'out-1', toNodeId: 'node-2', toPortId: 'in-1' },
  { id: 'conn-2', fromNodeId: 'node-1', fromPortId: 'out-1', toNodeId: 'node-3', toPortId: 'in-2' },
  { id: 'conn-3', fromNodeId: 'node-2', fromPortId: 'out-2', toNodeId: 'node-4', toPortId: 'in-3' },
  { id: 'conn-4', fromNodeId: 'node-3', fromPortId: 'out-3', toNodeId: 'node-4', toPortId: 'in-4' },
  { id: 'conn-5', fromNodeId: 'node-4', fromPortId: 'out-4', toNodeId: 'node-5', toPortId: 'in-5' },
];

function NodeEditor() {
  const [nodes, setNodes] = useState<NodeType[]>(demoNodes);
  const [connections] = useState<Connection[]>(demoConnections);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const canvasRef = useRef<HTMLDivElement>(null);

  const handleNodeDrag = useCallback((id: string, position: { x: number; y: number }) => {
    setNodes((prev) =>
      prev.map((node) =>
        node.id === id ? { ...node, position } : node
      )
    );
  }, []);

  const handleCanvasClick = (e: React.MouseEvent) => {
    if (e.target === canvasRef.current) {
      setSelectedNodeId(null);
    }
  };

  // Calculate connection lines
  const renderConnections = () => {
    return connections.map((conn) => {
      const fromNode = nodes.find((n) => n.id === conn.fromNodeId);
      const toNode = nodes.find((n) => n.id === conn.toNodeId);

      if (!fromNode || !toNode) return null;

      const fromIndex = fromNode.outputs.findIndex((p) => p.id === conn.fromPortId);
      const toIndex = toNode.inputs.findIndex((p) => p.id === conn.toPortId);

      const x1 = fromNode.position.x + 200;
      const y1 = fromNode.position.y + 40 + fromIndex * 24 + 12;
      const x2 = toNode.position.x;
      const y2 = toNode.position.y + 40 + toIndex * 24 + 12;

      const midX = (x1 + x2) / 2;

      return (
        <path
          key={conn.id}
          d={`M ${x1} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${x2} ${y2}`}
          stroke="var(--accent-primary)"
          strokeWidth="2"
          fill="none"
          style={{ pointerEvents: 'none' }}
        />
      );
    });
  };

  return (
    <div>
      <div className="project-list-header">
        <h1 className="project-list-title">Node Editor</h1>
        <div className="node-editor-toolbar">
          <button className="btn btn-secondary">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <polyline points="1 4 1 10 7 10" />
              <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
            </svg>
            Reset View
          </button>
          <button className="btn btn-primary">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
            Execute
          </button>
        </div>
      </div>

      <div className="node-editor">
        <div className="node-canvas" ref={canvasRef} onClick={handleCanvasClick}>
          <svg
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              height: '100%',
              pointerEvents: 'none',
            }}
          >
            {renderConnections()}
          </svg>

          {nodes.map((node) => (
            <Node
              key={node.id}
              node={node}
              selected={selectedNodeId === node.id}
              onSelect={() => setSelectedNodeId(node.id)}
              onDrag={handleNodeDrag}
            />
          ))}
        </div>
      </div>

      {selectedNodeId && (
        <div className="settings-section" style={{ marginTop: '16px' }}>
          <h2 className="settings-section-title">
            Node Properties: {nodes.find((n) => n.id === selectedNodeId)?.name}
          </h2>
          {Object.entries(nodes.find((n) => n.id === selectedNodeId)?.properties || {}).map(([key, prop]) => (
            <div key={key} className="settings-row">
              <div>
                <span className="settings-label">{prop.label}</span>
              </div>
              <div>
                {prop.type === 'number' && (
                  <input
                    type="number"
                    className="input"
                    style={{ width: '100px' }}
                    defaultValue={prop.default as number}
                    min={prop.min}
                    max={prop.max}
                  />
                )}
                {prop.type === 'select' && (
                  <select className="select">
                    {prop.options?.map((opt) => (
                      <option key={String(opt.value)} value={String(opt.value)}>
                        {opt.label}
                      </option>
                    ))}
                  </select>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default NodeEditor;

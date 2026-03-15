// Pool Web Dashboard Types

export interface Project {
  id: string;
  name: string;
  description: string;
  status: 'active' | 'archived' | 'draft';
  createdAt: string;
  updatedAt: string;
  shots: Shot[];
  workflows: Workflow[];
}

export interface Shot {
  id: string;
  projectId: string;
  name: string;
  duration: number; // in frames
  startTime: number; // in frames
  trackIndex: number;
  status: 'pending' | 'processing' | 'completed' | 'error';
  thumbnail?: string;
  metadata?: Record<string, unknown>;
}

export interface Workflow {
  id: string;
  projectId: string;
  name: string;
  description: string;
  nodes: Node[];
  connections: Connection[];
  status: 'idle' | 'running' | 'completed' | 'error';
  createdAt: string;
  updatedAt: string;
}

export interface Node {
  id: string;
  type: NodeType;
  name: string;
  position: { x: number; y: number };
  inputs: Port[];
  outputs: Port[];
  properties: Record<string, PropertyDefinition>;
  values: Record<string, unknown>;
}

export interface Port {
  id: string;
  name: string;
  type: 'input' | 'output';
  dataType: 'image' | 'video' | 'audio' | 'text' | 'any';
  required: boolean;
}

export interface PropertyDefinition {
  type: 'string' | 'number' | 'boolean' | 'select' | 'file';
  label: string;
  default?: unknown;
  options?: { label: string; value: unknown }[];
  min?: number;
  max?: number;
}

export interface Connection {
  id: string;
  fromNodeId: string;
  fromPortId: string;
  toNodeId: string;
  toPortId: string;
}

export type NodeType =
  | 'source'
  | 'transform'
  | 'filter'
  | 'output'
  | 'ai'
  | 'script';

export interface DashboardStats {
  totalProjects: number;
  activeProjects: number;
  totalShots: number;
  processingShots: number;
  storageUsed: number;
  storageTotal: number;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

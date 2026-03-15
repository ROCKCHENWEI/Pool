import { useState, useEffect, useCallback } from 'react';
import type { Project, Shot, Workflow, DashboardStats, ApiResponse } from '../types';

const API_BASE = '/api';

// Generic fetch wrapper
async function apiFetch<T>(
  endpoint: string,
  options?: RequestInit
): Promise<ApiResponse<T>> {
  try {
    const response = await fetch(`${API_BASE}${endpoint}`, {
      headers: {
        'Content-Type': 'application/json',
      },
      ...options,
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

// Projects hook
export function useProjects() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchProjects = useCallback(async () => {
    setLoading(true);
    const response = await apiFetch<Project[]>('/projects');
    if (response.success && response.data) {
      setProjects(response.data);
      setError(null);
    } else {
      setError(response.error || 'Failed to fetch projects');
    }
    setLoading(false);
  }, []);

  const createProject = useCallback(async (project: Partial<Project>) => {
    const response = await apiFetch<Project>('/projects', {
      method: 'POST',
      body: JSON.stringify(project),
    });
    if (response.success && response.data) {
      setProjects((prev) => [...prev, response.data!]);
      return response.data;
    }
    throw new Error(response.error);
  }, []);

  const updateProject = useCallback(async (id: string, updates: Partial<Project>) => {
    const response = await apiFetch<Project>(`/projects/${id}`, {
      method: 'PUT',
      body: JSON.stringify(updates),
    });
    if (response.success && response.data) {
      setProjects((prev) =>
        prev.map((p) => (p.id === id ? response.data! : p))
      );
      return response.data;
    }
    throw new Error(response.error);
  }, []);

  const deleteProject = useCallback(async (id: string) => {
    const response = await apiFetch<void>(`/projects/${id}`, {
      method: 'DELETE',
    });
    if (response.success) {
      setProjects((prev) => prev.filter((p) => p.id !== id));
      return true;
    }
    throw new Error(response.error);
  }, []);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  return {
    projects,
    loading,
    error,
    createProject,
    updateProject,
    deleteProject,
    refresh: fetchProjects,
  };
}

// Single project hook
export function useProject(id: string | undefined) {
  const [project, setProject] = useState<Project | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) {
      setLoading(false);
      return;
    }

    async function fetchProject() {
      setLoading(true);
      const response = await apiFetch<Project>(`/projects/${id}`);
      if (response.success && response.data) {
        setProject(response.data);
        setError(null);
      } else {
        setError(response.error || 'Failed to fetch project');
      }
      setLoading(false);
    }

    fetchProject();
  }, [id]);

  return { project, loading, error };
}

// Shots hook
export function useShots(projectId: string | undefined) {
  const [shots, setShots] = useState<Shot[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!projectId) {
      setLoading(false);
      return;
    }

    async function fetchShots() {
      setLoading(true);
      const response = await apiFetch<Shot[]>(`/projects/${projectId}/shots`);
      if (response.success && response.data) {
        setShots(response.data);
        setError(null);
      } else {
        setError(response.error || 'Failed to fetch shots');
      }
      setLoading(false);
    }

    fetchShots();
  }, [projectId]);

  const createShot = useCallback(
    async (shot: Partial<Shot>) => {
      const response = await apiFetch<Shot>(`/projects/${projectId}/shots`, {
        method: 'POST',
        body: JSON.stringify(shot),
      });
      if (response.success && response.data) {
        setShots((prev) => [...prev, response.data!]);
        return response.data;
      }
      throw new Error(response.error);
    },
    [projectId]
  );

  return { shots, loading, error, createShot };
}

// Workflows hook
export function useWorkflows(projectId: string | undefined) {
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!projectId) {
      setLoading(false);
      return;
    }

    async function fetchWorkflows() {
      setLoading(true);
      const response = await apiFetch<Workflow[]>(
        `/projects/${projectId}/workflows`
      );
      if (response.success && response.data) {
        setWorkflows(response.data);
        setError(null);
      } else {
        setError(response.error || 'Failed to fetch workflows');
      }
      setLoading(false);
    }

    fetchWorkflows();
  }, [projectId]);

  return { workflows, loading, error };
}

// Dashboard stats hook
export function useDashboardStats() {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchStats = useCallback(async () => {
    setLoading(true);
    const response = await apiFetch<DashboardStats>('/dashboard/stats');
    if (response.success && response.data) {
      setStats(response.data);
      setError(null);
    } else {
      setError(response.error || 'Failed to fetch stats');
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchStats();
  }, [fetchStats]);

  return { stats, loading, error, refresh: fetchStats };
}

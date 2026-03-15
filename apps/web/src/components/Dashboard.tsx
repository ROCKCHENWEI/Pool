import { useDashboardStats, useProjects } from '../hooks/usePool';

function Dashboard() {
  const { stats, loading: statsLoading } = useDashboardStats();
  const { projects, loading: projectsLoading } = useProjects();

  const isLoading = statsLoading || projectsLoading;

  if (isLoading) {
    return (
      <div className="dashboard">
        <div className="loading">
          <div className="loading-spinner"></div>
        </div>
      </div>
    );
  }

  return (
    <div className="dashboard">
      <div className="dashboard-header">
        <h1 className="dashboard-title">Dashboard</h1>
        <button className="btn btn-primary">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
          New Project
        </button>
      </div>

      <div className="stats-grid">
        <div className="stat-card">
          <div className="stat-card-title">Total Projects</div>
          <div className="stat-card-value">{stats?.totalProjects ?? projects.length}</div>
          <div className="stat-card-change positive">+2 this month</div>
        </div>
        <div className="stat-card">
          <div className="stat-card-title">Active Projects</div>
          <div className="stat-card-value">{stats?.activeProjects ?? projects.filter(p => p.status === 'active').length}</div>
          <div className="stat-card-change positive">+1 this week</div>
        </div>
        <div className="stat-card">
          <div className="stat-card-title">Total Shots</div>
          <div className="stat-card-value">{stats?.totalShots ?? 128}</div>
          <div className="stat-card-change positive">+24 today</div>
        </div>
        <div className="stat-card">
          <div className="stat-card-title">Processing</div>
          <div className="stat-card-value">{stats?.processingShots ?? 3}</div>
          <div className="stat-card-change">2 remaining</div>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="settings-section-title">Recent Projects</h2>
        {projects.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
              </svg>
            </div>
            <div className="empty-state-title">No projects yet</div>
            <div className="empty-state-description">
              Create your first project to get started
            </div>
            <button className="btn btn-primary">Create Project</button>
          </div>
        ) : (
          <div className="project-grid" style={{ marginTop: '16px' }}>
            {projects.slice(0, 3).map((project) => (
              <div key={project.id} className="project-card">
                <div className="project-card-thumbnail">
                  <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1">
                    <polygon points="5 3 19 12 5 21 5 3" />
                  </svg>
                </div>
                <div className="project-card-content">
                  <div className="project-card-name">{project.name}</div>
                  <div className="project-card-description">{project.description || 'No description'}</div>
                  <div className="project-card-footer">
                    <span className={`project-card-status ${project.status}`}>{project.status}</span>
                    <span className="project-card-date">
                      {new Date(project.updatedAt).toLocaleDateString()}
                    </span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default Dashboard;

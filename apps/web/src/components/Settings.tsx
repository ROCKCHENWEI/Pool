import { useState } from 'react';

interface SettingsState {
  theme: 'dark' | 'light';
  autoSave: boolean;
  autoSaveInterval: number;
  apiEndpoint: string;
  maxConcurrentJobs: number;
  cacheSize: number;
  notifications: boolean;
}

function Settings() {
  const [settings, setSettings] = useState<SettingsState>({
    theme: 'dark',
    autoSave: true,
    autoSaveInterval: 5,
    apiEndpoint: 'http://localhost:8080',
    maxConcurrentJobs: 4,
    cacheSize: 10,
    notifications: true,
  });

  const handleChange = <K extends keyof SettingsState>(key: K, value: SettingsState[K]) => {
    setSettings((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = () => {
    console.log('Saving settings:', settings);
    // API call would go here
    alert('Settings saved successfully!');
  };

  return (
    <div>
      <div className="project-list-header">
        <h1 className="project-list-title">Settings</h1>
        <button className="btn btn-primary" onClick={handleSave}>
          Save Changes
        </button>
      </div>

      <div className="settings-section">
        <h2 className="settings-section-title">Appearance</h2>
        <div className="settings-row">
          <div>
            <span className="settings-label">Theme</span>
            <p className="settings-description">Choose your preferred color scheme</p>
          </div>
          <select
            className="select"
            value={settings.theme}
            onChange={(e) => handleChange('theme', e.target.value as 'dark' | 'light')}
          >
            <option value="dark">Dark</option>
            <option value="light">Light</option>
          </select>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="settings-section-title">Editor</h2>
        <div className="settings-row">
          <div>
            <span className="settings-label">Auto Save</span>
            <p className="settings-description">Automatically save your work</p>
          </div>
          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={settings.autoSave}
              onChange={(e) => handleChange('autoSave', e.target.checked)}
              style={{ width: '16px', height: '16px' }}
            />
            {settings.autoSave ? 'On' : 'Off'}
          </label>
        </div>
        {settings.autoSave && (
          <div className="settings-row">
            <div>
              <span className="settings-label">Auto Save Interval</span>
              <p className="settings-description">Minutes between auto saves</p>
            </div>
            <input
              type="number"
              className="input"
              style={{ width: '80px' }}
              value={settings.autoSaveInterval}
              onChange={(e) => handleChange('autoSaveInterval', parseInt(e.target.value))}
              min={1}
              max={30}
            />
          </div>
        )}
      </div>

      <div className="settings-section">
        <h2 className="settings-section-title">Connection</h2>
        <div className="settings-row">
          <div>
            <span className="settings-label">API Endpoint</span>
            <p className="settings-description">Backend server URL</p>
          </div>
          <input
            type="text"
            className="input"
            style={{ width: '250px' }}
            value={settings.apiEndpoint}
            onChange={(e) => handleChange('apiEndpoint', e.target.value)}
            placeholder="http://localhost:8080"
          />
        </div>
        <div className="settings-row">
          <div>
            <span className="settings-label">Max Concurrent Jobs</span>
            <p className="settings-description">Maximum number of parallel processing jobs</p>
          </div>
          <input
            type="number"
            className="input"
            style={{ width: '80px' }}
            value={settings.maxConcurrentJobs}
            onChange={(e) => handleChange('maxConcurrentJobs', parseInt(e.target.value))}
            min={1}
            max={16}
          />
        </div>
      </div>

      <div className="settings-section">
        <h2 className="settings-section-title">Storage</h2>
        <div className="settings-row">
          <div>
            <span className="settings-label">Cache Size</span>
            <p className="settings-description">Maximum cache size in GB</p>
          </div>
          <input
            type="number"
            className="input"
            style={{ width: '80px' }}
            value={settings.cacheSize}
            onChange={(e) => handleChange('cacheSize', parseInt(e.target.value))}
            min={1}
            max={100}
          />
        </div>
        <div className="settings-row">
          <div>
            <span className="settings-label">Clear Cache</span>
            <p className="settings-description">Remove all cached files</p>
          </div>
          <button className="btn btn-secondary">Clear Cache</button>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="settings-section-title">Notifications</h2>
        <div className="settings-row">
          <div>
            <span className="settings-label">Enable Notifications</span>
            <p className="settings-description">Receive notifications for job completions</p>
          </div>
          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={settings.notifications}
              onChange={(e) => handleChange('notifications', e.target.checked)}
              style={{ width: '16px', height: '16px' }}
            />
            {settings.notifications ? 'On' : 'Off'}
          </label>
        </div>
      </div>

      <div className="settings-section">
        <h2 className="settings-section-title">About</h2>
        <div className="settings-row">
          <span className="settings-label">Version</span>
          <span style={{ color: 'var(--text-secondary)' }}>0.1.0</span>
        </div>
        <div className="settings-row">
          <span className="settings-label">Build</span>
          <span style={{ color: 'var(--text-secondary)' }}>2024.03.15</span>
        </div>
      </div>
    </div>
  );
}

export default Settings;

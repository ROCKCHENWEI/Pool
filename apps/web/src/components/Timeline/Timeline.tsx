import { useState } from 'react';
import ShotCard from './ShotCard';
import type { Shot } from '../../types';

interface Track {
  id: string;
  name: string;
  shots: Shot[];
}

// Demo data
const demoTracks: Track[] = [
  {
    id: '1',
    name: 'Video Track 1',
    shots: [
      { id: 's1', projectId: 'demo', name: 'Intro', duration: 100, startTime: 0, trackIndex: 0, status: 'completed' },
      { id: 's2', projectId: 'demo', name: 'Scene A', duration: 150, startTime: 100, trackIndex: 0, status: 'completed' },
      { id: 's3', projectId: 'demo', name: 'Scene B', duration: 120, startTime: 250, trackIndex: 0, status: 'processing' },
    ],
  },
  {
    id: '2',
    name: 'Video Track 2',
    shots: [
      { id: 's4', projectId: 'demo', name: 'Overlay', duration: 80, startTime: 50, trackIndex: 1, status: 'pending' },
    ],
  },
  {
    id: '3',
    name: 'Audio Track',
    shots: [
      { id: 's5', projectId: 'demo', name: 'Background Music', duration: 400, startTime: 0, trackIndex: 2, status: 'completed' },
    ],
  },
  {
    id: '4',
    name: 'Effects',
    shots: [
      { id: 's6', projectId: 'demo', name: 'Transition 1', duration: 20, startTime: 100, trackIndex: 3, status: 'completed' },
      { id: 's7', projectId: 'demo', name: 'Transition 2', duration: 20, startTime: 250, trackIndex: 3, status: 'pending' },
    ],
  },
];

function Timeline() {
  const [tracks] = useState<Track[]>(demoTracks);
  const [selectedShotId, setSelectedShotId] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [currentTime, setCurrentTime] = useState(0);

  const totalFrames = Math.max(...tracks.flatMap((t) => t.shots.map((s) => s.startTime + s.duration)));

  const handleZoomIn = () => setZoom((z) => Math.min(z + 0.2, 3));
  const handleZoomOut = () => setZoom((z) => Math.max(z - 0.2, 0.4));

  return (
    <div>
      <div className="project-list-header">
        <h1 className="project-list-title">Timeline Editor</h1>
        <div className="timeline-controls">
          <button className="btn btn-secondary" onClick={handleZoomOut}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
          </button>
          <button className="btn btn-secondary" onClick={handleZoomIn}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
          </button>
          <button className="btn btn-primary">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
            Play
          </button>
        </div>
      </div>

      <div className="timeline-container">
        <div className="timeline-header">
          <span className="timeline-title">Multi-Track Timeline</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
              Frame: {currentTime} / {totalFrames}
            </span>
            <span style={{ fontSize: '12px', color: 'var(--text-muted)' }}>
              Zoom: {Math.round(zoom * 100)}%
            </span>
          </div>
        </div>

        <div className="timeline-ruler">
          {Array.from({ length: Math.ceil(totalFrames / 50) + 1 }, (_, i) => (
            <div
              key={i}
              className="timeline-ruler-mark"
              style={{ width: `${50 * zoom}px` }}
            >
              {i * 50}
            </div>
          ))}
        </div>

        <div className="timeline-tracks">
          {tracks.map((track) => (
            <div key={track.id} className="timeline-track">
              <div className="timeline-track-label">{track.name}</div>
              <div className="timeline-track-content">
                {track.shots.map((shot) => (
                  <ShotCard
                    key={shot.id}
                    shot={shot}
                    zoom={zoom}
                    selected={selectedShotId === shot.id}
                    onClick={() => setSelectedShotId(shot.id)}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>

      {selectedShotId && (
        <div className="settings-section" style={{ marginTop: '16px' }}>
          <h2 className="settings-section-title">Selected Shot</h2>
          <div className="settings-row">
            <span className="settings-label">Name</span>
            <span>{tracks.flatMap((t) => t.shots).find((s) => s.id === selectedShotId)?.name}</span>
          </div>
          <div className="settings-row">
            <span className="settings-label">Status</span>
            <span>{tracks.flatMap((t) => t.shots).find((s) => s.id === selectedShotId)?.status}</span>
          </div>
          <div className="settings-row">
            <span className="settings-label">Duration</span>
            <span>{tracks.flatMap((t) => t.shots).find((s) => s.id === selectedShotId)?.duration} frames</span>
          </div>
        </div>
      )}
    </div>
  );
}

export default Timeline;

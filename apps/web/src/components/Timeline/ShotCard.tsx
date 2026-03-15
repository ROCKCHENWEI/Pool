import type { Shot } from '../../types';

interface ShotCardProps {
  shot: Shot;
  zoom: number;
  selected: boolean;
  onClick: () => void;
}

function ShotCard({ shot, zoom, selected, onClick }: ShotCardProps) {
  const left = shot.startTime * zoom;
  const width = shot.duration * zoom;

  const getStatusColor = (status: Shot['status']) => {
    switch (status) {
      case 'completed':
        return 'linear-gradient(135deg, #22c55e 0%, #16a34a 100%)';
      case 'processing':
        return 'linear-gradient(135deg, #3b82f6 0%, #2563eb 100%)';
      case 'error':
        return 'linear-gradient(135deg, #ef4444 0%, #dc2626 100%)';
      default:
        return 'linear-gradient(135deg, #6b7280 0%, #4b5563 100%)';
    }
  };

  return (
    <div
      className={`shot-card ${selected ? 'selected' : ''}`}
      style={{
        left: `${left}px`,
        width: `${Math.max(width, 40)}px`,
        background: getStatusColor(shot.status),
      }}
      onClick={onClick}
    >
      {width > 60 && (
        <>
          <div className="shot-card-thumbnail">
            {shot.thumbnail ? (
              <img src={shot.thumbnail} alt={shot.name} style={{ width: '100%', height: '100%', objectFit: 'cover', borderRadius: '4px' }} />
            ) : (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18" />
                <line x1="7" y1="2" x2="7" y2="22" />
                <line x1="17" y1="2" x2="17" y2="22" />
                <line x1="2" y1="12" x2="22" y2="12" />
                <line x1="2" y1="7" x2="7" y2="7" />
                <line x1="2" y1="17" x2="7" y2="17" />
                <line x1="17" y1="17" x2="22" y2="17" />
                <line x1="17" y1="7" x2="22" y2="7" />
              </svg>
            )}
          </div>
          <span className="shot-card-name">{shot.name}</span>
        </>
      )}
      {width <= 60 && width > 30 && (
        <span className="shot-card-name" style={{ fontSize: '10px' }}>
          {shot.name.slice(0, 3)}
        </span>
      )}
    </div>
  );
}

export default ShotCard;

# Pool Web Dashboard

React + TypeScript Web Dashboard for Pool project management.

## Features

- **Dashboard**: Overview of projects, stats, and recent activity
- **Project Management**: Create, view, and manage projects
- **Timeline Editor**: Multi-track timeline for shot management with zoom controls
- **Node Editor**: Visual node-based workflow editor with drag-and-drop
- **Settings**: Configure API endpoints, storage, and preferences

## Tech Stack

- React 18
- TypeScript
- React Router 6
- Vite

## Getting Started

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
```

## Project Structure

```
src/
├── main.tsx          # Application entry point
├── App.tsx           # Main app with routing
├── App.css           # Global styles (dark theme)
├── components/
│   ├── Layout.tsx    # Main layout wrapper
│   ├── Sidebar.tsx   # Navigation sidebar
│   ├── Dashboard.tsx # Dashboard view
│   ├── ProjectList.tsx # Project management
│   ├── Settings.tsx  # Settings page
│   ├── Timeline/
│   │   ├── Timeline.tsx   # Multi-track timeline
│   │   └── ShotCard.tsx   # Shot component
│   └── NodeEditor/
│       ├── NodeEditor.tsx # Node graph editor
│       └── Node.tsx       # Individual node
├── hooks/
│   └── usePool.ts    # API hooks for data fetching
└── types/
    └── index.ts      # TypeScript interfaces
```

## API Integration

The dashboard connects to the Pool backend API at `/api/*`. Configure the proxy in `vite.config.ts` for development.

## Development

The app uses a dark theme with CSS custom properties for theming. Key design tokens are defined in `App.css` under `:root`.

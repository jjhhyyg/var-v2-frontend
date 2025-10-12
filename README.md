# VAR Molten Pool Analysis System - Frontend

[简体中文](README.zh.md) | English

> Modern web interface built with Nuxt 4 for VAR molten pool video analysis system

## Tech Stack

- **Framework**: Nuxt 4.1.2 (Vue 3)
- **Language**: TypeScript 5.9
- **UI Library**: Nuxt UI 4.0
- **Charts**: ECharts 6.0 + Chart.js 4.5
- **Real-time Communication**: STOMP.js + SockJS
- **Export**: jsPDF + html2canvas
- **Package Manager**: pnpm (recommended) / npm

## Features

- Modern, responsive UI design
- Real-time task progress monitoring via WebSocket
- Interactive data visualization with ECharts
- Video upload with drag-and-drop support
- Analysis result display and export
- Dark mode support
- PDF report generation

## Prerequisites

- Node.js 18+ or 20+
- pnpm 8+ (or npm 9+)

## Quick Start

### 1. Install Dependencies

Using pnpm (recommended):

```bash
pnpm install
```

Using npm:

```bash
npm install
```

### 2. Configure Environment

Create a `.env` file in the frontend directory (or use the parent project's environment configuration):

```bash
# API Base URL
NUXT_PUBLIC_API_BASE_URL=http://localhost:8080
NUXT_PUBLIC_WS_BASE_URL=ws://localhost:8080
```

### 3. Run Development Server

```bash
pnpm dev
```

The application will start at http://localhost:3000

### 4. Build for Production

```bash
pnpm build
```

Preview production build:

```bash
pnpm preview
```

## Development

### Project Structure

```
frontend/
├── app/
│   ├── components/         # Vue components
│   ├── composables/        # Composable functions
│   ├── layouts/            # Layout components
│   ├── pages/              # Route pages
│   ├── plugins/            # Nuxt plugins
│   ├── utils/              # Utility functions
│   └── app.vue             # Root component
├── public/                 # Static assets
├── .nuxt/                  # Generated files (auto-generated)
├── nuxt.config.ts          # Nuxt configuration
├── tsconfig.json           # TypeScript configuration
└── package.json
```

### Key Technologies

#### Nuxt UI

This project uses [Nuxt UI](https://ui.nuxt.com/) for the component library, which provides:
- Beautiful, accessible components
- Built-in dark mode
- TypeScript support
- Tailwind CSS integration

#### Real-time Updates

WebSocket connection is established using STOMP.js and SockJS for real-time task progress updates:

```typescript
// Example usage in composables
const { connect, disconnect } = useWebSocket()
connect('/ws/progress', (message) => {
  console.log('Progress update:', message)
})
```

#### Data Visualization

ECharts is used for advanced charting capabilities:
- Line charts for time-series data
- Bar charts for statistics
- Custom visualizations for analysis results

### Code Quality

Run linter:

```bash
pnpm lint
```

Fix linting issues:

```bash
pnpm lint:fix
```

Type checking:

```bash
pnpm typecheck
```

## Configuration

### Nuxt Configuration

Key settings in `nuxt.config.ts`:

```typescript
export default defineNuxtConfig({
  modules: ['@nuxt/ui'],

  runtimeConfig: {
    public: {
      apiBaseUrl: process.env.NUXT_PUBLIC_API_BASE_URL || 'http://localhost:8080',
      wsBaseUrl: process.env.NUXT_PUBLIC_WS_BASE_URL || 'ws://localhost:8080'
    }
  }
})
```

### Environment Variables

- `NUXT_PUBLIC_API_BASE_URL` - Backend API base URL
- `NUXT_PUBLIC_WS_BASE_URL` - WebSocket server URL

## Docker Deployment

Build Docker image:

```bash
docker build -t var-frontend:latest .
```

Run with Docker:

```bash
docker run -p 3000:3000 \
  -e NUXT_PUBLIC_API_BASE_URL=http://api.example.com \
  var-frontend:latest
```

## Troubleshooting

### Port Already in Use

If port 3000 is already in use, specify a different port:

```bash
PORT=3001 pnpm dev
```

### API Connection Issues

Ensure the backend server is running and the `NUXT_PUBLIC_API_BASE_URL` is correctly set.

### WebSocket Connection Failed

Check that:
1. Backend WebSocket endpoint is accessible
2. `NUXT_PUBLIC_WS_BASE_URL` is correct
3. No firewall blocking WebSocket connections

## Performance Optimization

- Use `pnpm build` for optimized production builds
- Enable server-side rendering (SSR) for better SEO
- Lazy load components with `defineAsyncComponent`
- Optimize images in the `public/` directory

## Browser Support

- Chrome/Edge (latest 2 versions)
- Firefox (latest 2 versions)
- Safari (latest 2 versions)

## License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0) - see the [LICENSE](LICENSE) file for details.

**Important:** Any modified version of this software used over a network must make the source code available to users.

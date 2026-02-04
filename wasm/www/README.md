# emu86 Web Interface

React-based web interface for the emu86 8086 emulator with WebAssembly support.

## Development with Docker

### Prerequisites
- Docker and Docker Compose
- WASM package built in `pkg/` (run `../scripts/build.sh` from wasm directory)

### Quick Start

```bash
# Option 1: Using docker-compose directly
docker-compose up

# Option 2: Using the convenience script
./scripts/run-dev.sh
```

Visit http://localhost:3000

The development server includes:
- Automatic `npm install` on startup
- Live reload when source files change
- Hot module replacement (HMR)
- Volume mounting for instant updates

### What it does
The docker-compose setup:
1. Pulls the `node:20-alpine` image
2. Mounts your local directory (including `node_modules`)
3. Runs `npm install` to ensure dependencies are up to date
4. Starts the Vite dev server on port 3000

## Local Development (Without Docker)

If you prefer to develop locally without Docker:

```bash
# Install dependencies
npm install

# Build WASM package first (from wasm directory)
cd ..
sh scripts/build.sh
cd www

# Start development server
npm run dev
```

Visit http://localhost:3000

## Production Build

For production deployment:

```bash
# Build WASM package first (from wasm directory)
cd ..
sh scripts/build.sh
cd www

# Build the React app
npm run build

# The production build will be in the dist/ directory
# Serve it with any static file server (nginx, Apache, etc.)
```

## Project Structure

```
www/
├── index.html              # HTML entry point
├── public/                 # Static assets
├── pkg/                    # WASM build artifacts (built by ../scripts/build.sh)
├── src/
│   ├── main.jsx            # Entry point
│   ├── App.jsx             # Main app component
│   ├── components/         # React components
│   ├── hooks/              # Custom React hooks
│   └── styles/             # CSS files
├── docker-compose.yml      # Docker development setup
├── vite.config.js          # Vite configuration
└── package.json            # Dependencies
```

## Features

- React 18 with hooks
- Vite for fast development and optimized builds
- WebAssembly integration
- Keyboard and mouse input handling
- Floppy and hard drive management
- Pointer lock for infinite mouse movement
- Performance monitoring

## Architecture

### Components
- **EmulatorCanvas** - Canvas display with keyboard/mouse input
- **DriveControl** - Disk loading interface
- **BootControl** - Boot and reset controls
- **ExecutionControl** - Start/stop/step execution
- **StatusDisplay** - Status messages
- **RunningIndicator** - LED execution indicator
- **PerformanceDisplay** - CPU performance metrics

### Custom Hooks
- **useEmulator** - WASM emulator lifecycle management
- **usePointerLock** - Pointer lock state handling

## Development Notes

The original vanilla JavaScript implementation has been backed up as `index.html.backup`.

The React conversion maintains the same functionality while providing better code organization and maintainability through component-based architecture.

## Troubleshooting

### WASM Module Not Found
If you see errors about missing WASM files, ensure you've built the WASM package:
```bash
cd /path/to/emu86/wasm
sh scripts/build.sh
```

### Port Already in Use
If port 3000 is already in use, change the port mapping in [docker-compose.yml](docker-compose.yml):
```yaml
ports:
  - "3001:3000"  # Change host port to 3001
```

### node_modules Issues
If you encounter issues with node_modules:
```bash
# Remove node_modules and reinstall
rm -rf node_modules package-lock.json
docker-compose up
```

The docker-compose setup will reinstall dependencies automatically.

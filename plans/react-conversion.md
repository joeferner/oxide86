# React Conversion Plan for wasm/www

## Overview
Convert the existing vanilla JavaScript + WASM emulator web interface to a React application with proper component structure.

## Current State
- Single HTML file (index.html) with inline styles and vanilla JS
- WASM module loaded from pkg/ directory
- Features: disk loading, boot control, execution control, keyboard/mouse input

## Proposed Architecture

### Component Structure
```
src/
├── App.jsx                  # Main application component
├── components/
│   ├── EmulatorCanvas.jsx   # Canvas display and input handling
│   ├── DriveControl.jsx     # Floppy/HDD loading controls
│   ├── BootControl.jsx      # Boot and reset buttons
│   ├── ExecutionControl.jsx # Start/stop/step buttons
│   ├── StatusDisplay.jsx    # Status message display
│   ├── RunningIndicator.jsx # LED indicator
│   └── PerformanceDisplay.jsx # Performance metrics
├── hooks/
│   ├── useEmulator.js       # WASM computer instance management
│   ├── usePointerLock.js    # Pointer lock state management
│   └── useKeyboard.js       # Keyboard event handling
└── styles/
    └── App.css              # Global styles

public/
├── index.html               # Minimal HTML shell
└── pkg/                     # WASM build artifacts (copied)
```

### Key React Patterns
1. **State Management**: useState/useRef for emulator state, running status, pointer lock
2. **Side Effects**: useEffect for WASM initialization, event listeners, animation frames
3. **Custom Hooks**: Encapsulate emulator logic, keyboard/mouse handling
4. **Props**: Pass computer instance and handlers to child components

### Build Configuration
- Vite for fast dev server and production builds
- ES modules for WASM imports
- Copy WASM pkg/ to public/ directory during build

## Implementation Steps

1. **Initialize React Project**
   - Create package.json with React, ReactDOM, Vite
   - Setup Vite config to copy WASM files
   - Create basic index.html template

2. **Create Component Structure**
   - App.jsx as main container
   - Break UI into logical components
   - Extract styles to CSS modules or styled-components

3. **Implement Custom Hooks**
   - useEmulator: Initialize WASM, manage computer instance
   - usePointerLock: Handle pointer lock state
   - useKeyboard: Setup keyboard event listeners

4. **Convert Vanilla JS Logic**
   - File loading helpers
   - Event handlers to component methods
   - Animation frame loop in useEffect

5. **Create Dockerfile**
   - Multi-stage build: Node.js build, nginx serve
   - Copy WASM files to build
   - Production-ready nginx config

## Dockerfile Strategy

### Multi-Stage Build
```dockerfile
# Stage 1: Build React app
FROM node:20-alpine AS build
- Install dependencies
- Copy WASM pkg/ files
- Build production bundle

# Stage 2: Serve with nginx
FROM nginx:alpine
- Copy built React app
- Custom nginx config for SPA routing
```

### Benefits
- Minimal final image size (~25MB)
- Fast builds with layer caching
- Production-ready nginx serving
- Proper MIME types for WASM

## Testing Plan
1. Verify WASM module loads correctly
2. Test all disk loading operations
3. Confirm boot and execution controls work
4. Validate keyboard/mouse input
5. Check pointer lock functionality

## Rollback Plan
- Keep original index.html as index.html.backup
- Can revert to vanilla JS if issues arise

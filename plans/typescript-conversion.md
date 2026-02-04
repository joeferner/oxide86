# TypeScript Conversion Plan for React Application

## Overview
Convert the React application in wasm/www from JavaScript to TypeScript.

## Current State
- React application using JSX/JS files
- Vite as build tool
- @types/react and @types/react-dom already installed

## Files to Convert
1. Configuration:
   - vite.config.js → vite.config.ts
   - Create tsconfig.json

2. Entry points:
   - src/main.jsx → src/main.tsx
   - Update index.html reference

3. Hooks:
   - src/hooks/useEmulator.js → src/hooks/useEmulator.ts
   - src/hooks/usePointerLock.js → src/hooks/usePointerLock.ts

4. Components:
   - src/App.jsx → src/App.tsx
   - src/components/EmulatorCanvas.jsx → src/components/EmulatorCanvas.tsx
   - src/components/DriveControl.jsx → src/components/DriveControl.tsx
   - src/components/BootControl.jsx → src/components/BootControl.tsx
   - src/components/ExecutionControl.jsx → src/components/ExecutionControl.tsx
   - src/components/StatusDisplay.jsx → src/components/StatusDisplay.tsx
   - src/components/RunningIndicator.jsx → src/components/RunningIndicator.tsx
   - src/components/PerformanceDisplay.jsx → src/components/PerformanceDisplay.tsx
   - src/components/InfoBox.jsx → src/components/InfoBox.tsx

5. Type definitions:
   - Create src/types/wasm.d.ts for WASM module types

## Implementation Steps
1. Create tsconfig.json with proper React + Vite configuration
2. Create type definitions for WASM module (Emu86Computer)
3. Convert vite.config.js to TypeScript
4. Convert hooks with proper type annotations
5. Convert components with proper prop types
6. Update index.html to reference main.tsx
7. Install typescript if needed
8. Test build

## TypeScript Types Needed
- Emu86Computer interface (WASM module)
- Performance interface { target: number, actual: number }
- Component prop types for all components
- Ref types for canvas elements

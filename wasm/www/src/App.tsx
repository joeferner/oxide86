import { useCallback, useRef } from 'react'
import { useEmulator } from './hooks/useEmulator'
import { usePointerLock } from './hooks/usePointerLock'
import { EmulatorCanvas } from './components/EmulatorCanvas'
import { InfoBox } from './components/InfoBox'
import { DriveControl } from './components/DriveControl'
import { BootControl } from './components/BootControl'
import { ExecutionControl } from './components/ExecutionControl'
import { StatusDisplay } from './components/StatusDisplay'
import { RunningIndicator } from './components/RunningIndicator'
import { PerformanceDisplay } from './components/PerformanceDisplay'

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { isLocked } = usePointerLock(canvasRef)

  const {
    computer,
    status,
    setStatus,
    isRunning,
    performance,
    startExecution,
    stopExecution,
    stepExecution,
    boot,
    reset,
  } = useEmulator(canvasRef)

  const handleStatusUpdate = useCallback((message: string) => {
    setStatus(message)
  }, [setStatus])

  const handleBootA = useCallback(() => {
    boot(0x00)
  }, [boot])

  const handleBootC = useCallback(() => {
    boot(0x80)
  }, [boot])

  return (
    <>
      <h1>emu86 - Intel 8086 Emulator</h1>

      <InfoBox isPointerLocked={isLocked} />

      <div id="main-container">
        <EmulatorCanvas
          ref={canvasRef}
          computer={computer}
          onStatusUpdate={handleStatusUpdate}
        />

        <div className="controls">
          <DriveControl
            computer={computer}
            onStatusUpdate={handleStatusUpdate}
          />

          <BootControl
            onBootA={handleBootA}
            onBootC={handleBootC}
            onReset={reset}
          />

          <ExecutionControl
            isRunning={isRunning}
            onStart={startExecution}
            onStop={stopExecution}
            onStep={stepExecution}
          />

          <StatusDisplay status={status} />

          <RunningIndicator isRunning={isRunning} />

          <PerformanceDisplay performance={performance} />
        </div>
      </div>
    </>
  )
}

export default App

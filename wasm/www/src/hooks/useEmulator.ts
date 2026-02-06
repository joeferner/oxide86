import { useState, useEffect, useRef, RefObject } from 'react'
import wasmInit, { Emu86Computer } from '../../pkg/emu86_wasm'

interface Performance {
  target: number;
  actual: number;
}

interface UseEmulatorReturn {
  computer: Emu86Computer | null;
  status: string;
  setStatus: (status: string) => void;
  isRunning: boolean;
  performance: Performance;
  startExecution: () => void;
  stopExecution: () => void;
  stepExecution: () => void;
  loadProgram: (data: Uint8Array, segment: number, offset: number) => void;
  reset: () => void;
  bootAndStart: () => void;
}

export function useEmulator(canvasRef: RefObject<HTMLCanvasElement>, bootDrive: number): UseEmulatorReturn {
  const [computer, setComputer] = useState<Emu86Computer | null>(null)
  const [status, setStatus] = useState('Initializing...')
  const [isRunning, setIsRunning] = useState(false)
  const [performance, setPerformance] = useState<Performance>({ target: 0, actual: 0 })
  const animationFrameRef = useRef<number | null>(null)
  const initializedRef = useRef(false)
  const isRunningRef = useRef(false)

  useEffect(() => {
    // Prevent double initialization (React StrictMode runs effects twice in dev)
    if (initializedRef.current) {
      return
    }

    let mounted = true

    const initEmulator = async () => {
      // Wait for canvas to be available
      if (!canvasRef?.current) {
        // Retry after a short delay
        if (mounted) {
          setTimeout(initEmulator, 100)
        }
        return
      }

      // Double-check we haven't already initialized
      if (initializedRef.current) {
        return
      }

      try {
        initializedRef.current = true

        // Initialize WASM module
        await wasmInit()

        const comp = new Emu86Computer('display')
        comp.attach_serial_mouse_com1()

        if (mounted) {
          setComputer(comp)
          setStatus('Emulator initialized. Serial mouse attached to COM1. Load disk images to begin.')
        }
      } catch (e) {
        initializedRef.current = false
        if (mounted) {
          setStatus(`Initialization error: ${e}`)
          console.error(e)
        }
      }
    }

    initEmulator()

    return () => {
      mounted = false
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current)
      }
      // Reset flag on cleanup so remount can reinitialize
      initializedRef.current = false
    }
  }, [canvasRef])

  const updatePerformance = () => {
    if (computer) {
      try {
        const target = computer.get_target_mhz()
        const actual = computer.get_actual_mhz()
        setPerformance({ target, actual })
      } catch (e) {
        // Silently fail if not ready
      }
    }
  }

  const startExecution = () => {
    if (!computer || isRunningRef.current) return

    isRunningRef.current = true
    setIsRunning(true)
    setStatus('Running... (click on display for keyboard input)')

    const frame = () => {
      if (!isRunningRef.current) return

      try {
        const stillRunning = computer.run_for_ms(16, window.performance.now())
        updatePerformance()

        if (!stillRunning) {
          isRunningRef.current = false
          setIsRunning(false)
          setStatus('CPU halted')
          return
        }
        animationFrameRef.current = requestAnimationFrame(frame)
      } catch (e) {
        isRunningRef.current = false
        setIsRunning(false)
        setStatus(`Execution error: ${e}`)
        console.error(e)
      }
    }

    updatePerformance()
    animationFrameRef.current = requestAnimationFrame(frame)
  }

  const stopExecution = () => {
    if (!isRunningRef.current) return

    isRunningRef.current = false
    setIsRunning(false)
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current)
      animationFrameRef.current = null
    }
    setStatus('Stopped')
  }

  const stepExecution = () => {
    if (!computer) return

    try {
      const stillRunning = computer.step()
      setStatus(stillRunning ? 'Stepped 1 instruction' : 'CPU halted')
    } catch (e) {
      setStatus(`Step error: ${e}`)
      console.error(e)
    }
  }

  const bootAndStart = () => {
    if (!computer || isRunningRef.current) return

    try {
      // Boot from the selected drive
      computer.boot(bootDrive)
      const driveName = bootDrive === 0x00 ? 'floppy A:' : 'hard drive C:'
      setStatus(`Booted from ${driveName}, starting execution...`)

      // Start execution
      isRunningRef.current = true
      setIsRunning(true)

      const frame = () => {
        if (!isRunningRef.current) return

        try {
          const stillRunning = computer.run_for_ms(16, window.performance.now())
          updatePerformance()

          if (!stillRunning) {
            isRunningRef.current = false
            setIsRunning(false)
            setStatus('CPU halted')
            return
          }
          animationFrameRef.current = requestAnimationFrame(frame)
        } catch (e) {
          isRunningRef.current = false
          setIsRunning(false)
          setStatus(`Execution error: ${e}`)
          console.error(e)
        }
      }

      updatePerformance()
      animationFrameRef.current = requestAnimationFrame(frame)
    } catch (e) {
      setStatus(`Boot error: ${e}`)
      console.error(e)
    }
  }

  const loadProgram = (data: Uint8Array, segment: number, offset: number) => {
    if (!computer) return

    try {
      computer.load_program(data, segment, offset)
      setStatus(`Loaded program: ${data.length} bytes at ${segment.toString(16).toUpperCase().padStart(4, '0')}:${offset.toString(16).toUpperCase().padStart(4, '0')}`)
    } catch (e) {
      setStatus(`Load error: ${e}`)
      console.error(e)
    }
  }

  const reset = () => {
    if (!computer) return

    try {
      computer.reset()
      stopExecution()
      setStatus('Computer reset')
    } catch (e) {
      setStatus(`Reset error: ${e}`)
      console.error(e)
    }
  }

  return {
    computer,
    status,
    setStatus,
    isRunning,
    performance,
    startExecution,
    stopExecution,
    stepExecution,
    loadProgram,
    reset,
    bootAndStart,
  }
}

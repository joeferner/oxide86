import { useRef, useEffect, forwardRef } from 'react'
import { usePointerLock } from '../hooks/usePointerLock'
import { Emu86Computer } from '../../pkg/emu86_wasm'
import styles from './EmulatorCanvas.module.scss'

interface EmulatorCanvasProps {
  computer: Emu86Computer | null;
  onStatusUpdate: (message: string) => void;
}

export const EmulatorCanvas = forwardRef<HTMLCanvasElement, EmulatorCanvasProps>(
  function EmulatorCanvas({ computer, onStatusUpdate }, forwardedRef) {
    const canvasRef = (forwardedRef as React.RefObject<HTMLCanvasElement>) || useRef<HTMLCanvasElement>(null)
    const { isLocked, requestLock, exitLock } = usePointerLock(canvasRef)

    useEffect(() => {
      if (isLocked) {
        onStatusUpdate('Mouse locked to canvas (press F12 to exit)')
      } else {
        onStatusUpdate('Mouse unlocked (click canvas to lock)')
      }
    }, [isLocked, onStatusUpdate])

    useEffect(() => {
      const canvas = canvasRef.current
      if (!canvas || !computer) return

      const preventDefaultKeys = [
        'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight',
        'Backspace', 'Tab', 'Space',
        'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12'
      ]

      const handleKeyDown = (event: KeyboardEvent) => {
        if (event.repeat) return

        if (event.code === 'F12' && isLocked) {
          event.preventDefault()
          exitLock()
          return
        }

        if (preventDefaultKeys.includes(event.code) || event.altKey) {
          event.preventDefault()
        }

        try {
          computer.handle_key_event(
            event.code,
            event.key,
            event.shiftKey,
            event.ctrlKey,
            event.altKey
          )
        } catch (e) {
          console.error('Keyboard event error:', e)
        }
      }

      const handleMouseMove = (event: MouseEvent) => {
        try {
          if (isLocked) {
            computer.handle_mouse_delta(event.movementX, event.movementY)
          } else {
            computer.handle_mouse_move(event.offsetX, event.offsetY)
          }
        } catch (e) {
          console.error('Mouse move error:', e)
        }
      }

      const handleMouseDown = (event: MouseEvent) => {
        event.preventDefault()
        canvas.focus()
        try {
          computer.handle_mouse_button(event.button, true)
        } catch (e) {
          console.error('Mouse button error:', e)
        }
      }

      const handleMouseUp = (event: MouseEvent) => {
        try {
          computer.handle_mouse_button(event.button, false)
        } catch (e) {
          console.error('Mouse button error:', e)
        }
      }

      const handleClick = () => {
        canvas.focus()
        requestLock()
      }

      canvas.addEventListener('keydown', handleKeyDown)
      canvas.addEventListener('mousemove', handleMouseMove)
      canvas.addEventListener('mousedown', handleMouseDown)
      canvas.addEventListener('mouseup', handleMouseUp)
      canvas.addEventListener('click', handleClick)

      return () => {
        canvas.removeEventListener('keydown', handleKeyDown)
        canvas.removeEventListener('mousemove', handleMouseMove)
        canvas.removeEventListener('mousedown', handleMouseDown)
        canvas.removeEventListener('mouseup', handleMouseUp)
        canvas.removeEventListener('click', handleClick)
      }
    }, [computer, isLocked, requestLock, exitLock])

    return (
      <div className={styles.container}>
        <canvas
          ref={canvasRef}
          className={styles.canvas}
          id="display"
          width="640"
          height="400"
          tabIndex={0}
        />
      </div>
    )
  }
)

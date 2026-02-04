import { useState, useEffect, useCallback } from 'react'

export function usePointerLock(canvasRef) {
  const [isLocked, setIsLocked] = useState(false)

  const requestLock = useCallback(() => {
    if (canvasRef.current) {
      canvasRef.current.requestPointerLock()
    }
  }, [canvasRef])

  const exitLock = useCallback(() => {
    if (document.pointerLockElement === canvasRef.current) {
      document.exitPointerLock()
    }
  }, [canvasRef])

  useEffect(() => {
    const handleLockChange = () => {
      setIsLocked(document.pointerLockElement === canvasRef.current)
    }

    const handleLockError = () => {
      console.error('Pointer lock error')
    }

    document.addEventListener('pointerlockchange', handleLockChange)
    document.addEventListener('pointerlockerror', handleLockError)

    return () => {
      document.removeEventListener('pointerlockchange', handleLockChange)
      document.removeEventListener('pointerlockerror', handleLockError)
    }
  }, [canvasRef])

  return {
    isLocked,
    requestLock,
    exitLock,
  }
}

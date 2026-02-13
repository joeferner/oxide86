import { useState, useEffect, useCallback, RefObject } from 'react';

interface UsePointerLockReturn {
    isLocked: boolean;
    requestLock: () => void;
    exitLock: () => void;
}

export function usePointerLock(canvasRef: RefObject<HTMLCanvasElement>): UsePointerLockReturn {
    const [isLocked, setIsLocked] = useState(false);

    const requestLock = useCallback(() => {
        if (canvasRef.current) {
            void canvasRef.current.requestPointerLock();
        }
    }, [canvasRef]);

    const exitLock = useCallback(() => {
        if (document.pointerLockElement === canvasRef.current) {
            document.exitPointerLock();
        }
    }, [canvasRef]);

    useEffect(() => {
        const handleLockChange = (): void => {
            setIsLocked(document.pointerLockElement === canvasRef.current);
        };

        const handleLockError = (): void => {
            console.error('Pointer lock error');
        };

        document.addEventListener('pointerlockchange', handleLockChange);
        document.addEventListener('pointerlockerror', handleLockError);

        return () => {
            document.removeEventListener('pointerlockchange', handleLockChange);
            document.removeEventListener('pointerlockerror', handleLockError);
        };
    }, [canvasRef]);

    return {
        isLocked,
        requestLock,
        exitLock,
    };
}

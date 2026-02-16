import { signal } from '@preact/signals-react';

export const isLocked = signal(false);

let _canvas: HTMLCanvasElement | null = null;

export function initPointerLock(canvas: HTMLCanvasElement): () => void {
    _canvas = canvas;

    const handleLockChange = (): void => {
        isLocked.value = document.pointerLockElement === _canvas;
    };

    const handleLockError = (): void => {
        console.error('Pointer lock error');
    };

    document.addEventListener('pointerlockchange', handleLockChange);
    document.addEventListener('pointerlockerror', handleLockError);

    return () => {
        document.removeEventListener('pointerlockchange', handleLockChange);
        document.removeEventListener('pointerlockerror', handleLockError);
        _canvas = null;
    };
}

export function requestLock(): void {
    if (_canvas) {
        void _canvas.requestPointerLock();
    }
}

export function exitLock(): void {
    if (_canvas && document.pointerLockElement === _canvas) {
        document.exitPointerLock();
    }
}

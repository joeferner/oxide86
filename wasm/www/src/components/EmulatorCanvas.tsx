import { useRef, useEffect, forwardRef } from 'react';
import { useSignalEffect } from '@preact/signals-react';
import { Oxide86Computer } from '../../pkg/oxide86_wasm';
import { initPointerLock, isLocked, requestLock, exitLock } from '../pointerLockState';
import { status } from '../emulatorState';
import styles from './EmulatorCanvas.module.scss';

interface EmulatorCanvasProps {
    computer: Oxide86Computer | null;
}

export const EmulatorCanvas = forwardRef<HTMLCanvasElement, EmulatorCanvasProps>(function EmulatorCanvas(
    { computer },
    forwardedRef
): React.ReactElement {
    const localRef = useRef<HTMLCanvasElement>(null);
    const canvasRef = (forwardedRef as React.RefObject<HTMLCanvasElement> | null) ?? localRef;

    // Register pointer lock listeners and clean up on unmount
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) {
            return;
        }
        return initPointerLock(canvas);
    }, [canvasRef]);

    // Update status when pointer lock state changes
    useSignalEffect(() => {
        if (isLocked.value) {
            status.value = 'Mouse locked to canvas (press F12 to exit)';
        } else {
            status.value = 'Mouse unlocked (click canvas to lock)';
        }
    });

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas || !computer) {
            return;
        }

        const preventDefaultKeys = [
            'ArrowUp',
            'ArrowDown',
            'ArrowLeft',
            'ArrowRight',
            'Backspace',
            'Tab',
            'Space',
            'F1',
            'F2',
            'F3',
            'F4',
            'F5',
            'F6',
            'F7',
            'F8',
            'F9',
            'F10',
            'F11',
            'F12',
        ];

        const handleKeyDown = (event: KeyboardEvent): void => {
            if (event.code === 'F12' && isLocked.value) {
                event.preventDefault();
                exitLock();
                return;
            }

            if (preventDefaultKeys.includes(event.code) || event.altKey) {
                event.preventDefault();
            }

            try {
                computer.handle_key_event(event.code, event.key, event.shiftKey, event.ctrlKey, event.altKey, true);
            } catch (e) {
                console.error('Keyboard event error:', e);
            }
        };

        const handleKeyUp = (event: KeyboardEvent): void => {
            if (preventDefaultKeys.includes(event.code) || event.altKey) {
                event.preventDefault();
            }

            try {
                computer.handle_key_event(event.code, event.key, event.shiftKey, event.ctrlKey, event.altKey, false);
            } catch (e) {
                console.error('Keyboard event error:', e);
            }
        };

        const handleMouseMove = (event: MouseEvent): void => {
            try {
                if (isLocked.value) {
                    computer.handle_mouse_delta(event.movementX, event.movementY);
                } else {
                    computer.handle_mouse_move(event.offsetX, event.offsetY);
                }
            } catch (e) {
                console.error('Mouse move error:', e);
            }
        };

        const handleMouseDown = (event: MouseEvent): void => {
            event.preventDefault();
            canvas.focus();
            try {
                computer.handle_mouse_button(event.button, true);
            } catch (e) {
                console.error('Mouse button error:', e);
            }
        };

        const handleMouseUp = (event: MouseEvent): void => {
            try {
                computer.handle_mouse_button(event.button, false);
            } catch (e) {
                console.error('Mouse button error:', e);
            }
        };

        const handleClick = (): void => {
            canvas.focus();
            requestLock();
        };

        canvas.addEventListener('keydown', handleKeyDown);
        canvas.addEventListener('keyup', handleKeyUp);
        canvas.addEventListener('mousemove', handleMouseMove);
        canvas.addEventListener('mousedown', handleMouseDown);
        canvas.addEventListener('mouseup', handleMouseUp);
        canvas.addEventListener('click', handleClick);

        return () => {
            canvas.removeEventListener('keydown', handleKeyDown);
            canvas.removeEventListener('keyup', handleKeyUp);
            canvas.removeEventListener('mousemove', handleMouseMove);
            canvas.removeEventListener('mousedown', handleMouseDown);
            canvas.removeEventListener('mouseup', handleMouseUp);
            canvas.removeEventListener('click', handleClick);
        };
    }, [computer, canvasRef]);

    return (
        <div className={styles.container}>
            <canvas ref={canvasRef} className={styles.canvas} id="display" width="640" height="400" tabIndex={0} />
        </div>
    );
});

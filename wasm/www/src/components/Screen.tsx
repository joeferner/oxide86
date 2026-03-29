import React, { useEffect, useRef } from 'react';
import { useSignalEffect } from '@preact/signals-react';
import { state } from '../state';
import { KEY_MAP } from '../keycodes';

export function Screen(): React.ReactElement {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) { return; }
        const ctx = canvas.getContext('2d');
        if (!ctx) { return; }
        ctx.fillStyle = '#000';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
    }, []);

    // Reactive RAF loop — reruns automatically when state.computer changes.
    useSignalEffect(() => {
        const computer = state.computer.value;
        if (!computer) {
            return;
        }

        const canvas = canvasRef.current;
        if (!canvas) {
            return;
        }

        const ctx = canvas.getContext('2d');
        if (!ctx) {
            return;
        }

        let raf = 0;

        const tick = (): void => {
            const result = computer.run_for_cycles(100_000);

            const error = computer.get_last_error();
            if (error) {
                state.setStatus('Error', error);
                return;
            }

            const frame = computer.render_frame();

            if (canvas.width !== frame.width || canvas.height !== frame.height) {
                canvas.width = frame.width;
                canvas.height = frame.height;
            }

            ctx.putImageData(new ImageData(new Uint8ClampedArray(frame.data), frame.width, frame.height), 0, 0);

            if (!result.halted) {
                raf = requestAnimationFrame(tick);
            } else {
                state.setStatus('Halted');
            }
        };

        raf = requestAnimationFrame(tick);
        return () => {
            cancelAnimationFrame(raf);
        };
    });

    // Keyboard input — always active regardless of power state.
    useEffect(() => {
        const onKeyDown = (e: KeyboardEvent): void => {
            const scanCode = KEY_MAP[e.code];
            if (scanCode === undefined) {
                return;
            }
            e.preventDefault();
            state.computer.value?.push_key_event(scanCode, true);
        };

        const onKeyUp = (e: KeyboardEvent): void => {
            const scanCode = KEY_MAP[e.code];
            if (scanCode === undefined) {
                return;
            }
            e.preventDefault();
            state.computer.value?.push_key_event(scanCode, false);
        };

        window.addEventListener('keydown', onKeyDown);
        window.addEventListener('keyup', onKeyUp);
        return () => {
            window.removeEventListener('keydown', onKeyDown);
            window.removeEventListener('keyup', onKeyUp);
        };
    }, []);

    // Mouse capture via Pointer Lock.
    useEffect(() => {
        const onMouseMove = (e: MouseEvent): void => {
            state.computer.value?.push_mouse_event(e.movementX, e.movementY, e.buttons);
        };

        const onPointerLockChange = (): void => {
            if (document.pointerLockElement === canvasRef.current) {
                document.addEventListener('mousemove', onMouseMove);
            } else {
                document.removeEventListener('mousemove', onMouseMove);
            }
        };

        document.addEventListener('pointerlockchange', onPointerLockChange);
        return () => {
            document.removeEventListener('pointerlockchange', onPointerLockChange);
            document.removeEventListener('mousemove', onMouseMove);
        };
    }, []);

    const onCanvasClick = (): void => {
        void canvasRef.current?.requestPointerLock();
    };

    return (
        <canvas
            ref={canvasRef}
            width={640}
            height={400}
            onClick={onCanvasClick}
            style={{ imageRendering: 'pixelated', display: 'block' }}
        />
    );
}

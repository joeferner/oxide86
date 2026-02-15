import { useState, useEffect, useRef, RefObject, useCallback } from 'react';
import wasmInit, { Emu86Computer } from '../../pkg/emu86_wasm';
import { EmulatorConfig, DEFAULT_CONFIG } from '../components/ConfigDialog.consts';

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
    loadProgram: (data: Uint8Array, segment: number, offset: number) => void;
    reset: () => void;
    bootAndStart: () => void;
    applyConfig: (config: EmulatorConfig) => void;
}

export function useEmulator(
    canvasRef: RefObject<HTMLCanvasElement>,
    bootDrive: number,
    initialConfig: EmulatorConfig = DEFAULT_CONFIG
): UseEmulatorReturn {
    const [computer, setComputer] = useState<Emu86Computer | null>(null);
    const [status, setStatus] = useState('Initializing...');
    const [isRunning, setIsRunning] = useState(false);
    const [performance, setPerformance] = useState<Performance>({ target: 0, actual: 0 });
    const animationFrameRef = useRef<number | null>(null);
    const wasmInitializedRef = useRef(false);
    const isRunningRef = useRef(false);
    const configRef = useRef<EmulatorConfig>(initialConfig);
    // Tracks which gamepad index is assigned to which joystick slot (0=A, 1=B)
    const gamepadSlotsRef = useRef<Map<number, number>>(new Map());
    // Ref to current computer for use in event handlers (avoids stale closure)
    const computerRef = useRef<Emu86Computer | null>(null);

    // Poll connected gamepads and push state to emulator
    const pollGamepads = (comp: Emu86Computer): void => {
        const config = configRef.current;
        if (!config.joystickA && !config.joystickB) {
            return;
        }
        const gamepads = navigator.getGamepads();
        for (const [gpIndex, slot] of gamepadSlotsRef.current) {
            const gp = gamepads[gpIndex];
            if (gp) {
                const x = gp.axes.length > 0 ? (gp.axes[0] + 1) / 2 : 0.5;
                const y = gp.axes.length > 1 ? (gp.axes[1] + 1) / 2 : 0.5;
                try {
                    comp.handle_gamepad_axis(slot, 0, x);
                    comp.handle_gamepad_axis(slot, 1, y);
                    comp.handle_gamepad_button(slot, 0, gp.buttons[0]?.pressed ?? false);
                    comp.handle_gamepad_button(slot, 1, gp.buttons[1]?.pressed ?? false);
                } catch (_e) {
                    // ignore
                }
            }
        }
    };

    // Assign a connected gamepad to the first available enabled slot
    const assignGamepad = (gamepadIndex: number): void => {
        const config = configRef.current;
        const assigned = new Set(gamepadSlotsRef.current.values());
        if (config.joystickA && !assigned.has(0)) {
            gamepadSlotsRef.current.set(gamepadIndex, 0);
            computerRef.current?.gamepad_connected(0, true);
        } else if (config.joystickB && !assigned.has(1)) {
            gamepadSlotsRef.current.set(gamepadIndex, 1);
            computerRef.current?.gamepad_connected(1, true);
        }
    };

    const createComputer = useCallback((config: EmulatorConfig): Emu86Computer => {
        const comp = new Emu86Computer({
            canvas_id: 'display',
            cpu_type: config.cpuType,
            memory_kb: config.memoryKb,
            clock_mhz: config.clockMhz,
            video_card: config.videoCard,
            com1_device: config.com1Device,
            com2_device: config.com2Device,
        });
        return comp;
    }, []);

    useEffect(() => {
        let mounted = true;

        const initEmulator = async (): Promise<void> => {
            // Wait for canvas to be available
            if (!canvasRef?.current) {
                if (mounted) {
                    setTimeout(initEmulator, 100);
                }
                return;
            }

            try {
                // Initialize WASM module only once
                if (!wasmInitializedRef.current) {
                    await wasmInit();
                    wasmInitializedRef.current = true;
                }

                const comp = createComputer(configRef.current);

                if (mounted) {
                    setComputer(comp);
                    setStatus('Emulator initialized. Load disk images to begin.');
                }
            } catch (e) {
                if (mounted) {
                    setStatus(`Initialization error: ${e}`);
                    console.error(e);
                }
            }
        };

        void initEmulator();

        return () => {
            mounted = false;
            if (animationFrameRef.current) {
                cancelAnimationFrame(animationFrameRef.current);
            }
        };
    }, [canvasRef, createComputer]);

    // Keep computerRef in sync with computer state for use in event handlers
    useEffect(() => {
        computerRef.current = computer;
    }, [computer]);

    // Handle gamepad connect/disconnect events and scan already-connected gamepads
    useEffect(() => {
        const handleConnected = (e: GamepadEvent): void => {
            assignGamepad(e.gamepad.index);
        };
        const handleDisconnected = (e: GamepadEvent): void => {
            const slot = gamepadSlotsRef.current.get(e.gamepad.index);
            if (slot !== undefined) {
                computerRef.current?.gamepad_connected(slot, false);
                gamepadSlotsRef.current.delete(e.gamepad.index);
            }
        };
        window.addEventListener('gamepadconnected', handleConnected);
        window.addEventListener('gamepaddisconnected', handleDisconnected);
        // Scan gamepads already connected before this effect ran
        const gamepads = navigator.getGamepads();
        for (const gp of gamepads) {
            if (gp && !gamepadSlotsRef.current.has(gp.index)) {
                assignGamepad(gp.index);
            }
        }
        return () => {
            window.removeEventListener('gamepadconnected', handleConnected);
            window.removeEventListener('gamepaddisconnected', handleDisconnected);
        };
    }, [computer]); // computer changes whenever config changes (applyConfig recreates it)

    const updatePerformance = (): void => {
        if (computer) {
            try {
                const target = computer.get_target_mhz();
                const actual = computer.get_actual_mhz();
                setPerformance({ target, actual });
            } catch (_e) {
                // Silently fail if not ready
            }
        }
    };

    const startExecution = (): void => {
        if (!computer || isRunningRef.current) {
            return;
        }

        isRunningRef.current = true;
        setIsRunning(true);
        setStatus('Running... (click on display for keyboard input)');

        const frame = (): void => {
            if (!isRunningRef.current) {
                return;
            }

            try {
                pollGamepads(computer);
                const stillRunning = computer.run_for_ms(16, window.performance.now());
                updatePerformance();

                if (!stillRunning) {
                    isRunningRef.current = false;
                    setIsRunning(false);
                    setStatus('CPU halted');
                    return;
                }
                animationFrameRef.current = requestAnimationFrame(frame);
            } catch (e) {
                isRunningRef.current = false;
                setIsRunning(false);
                setStatus(`Execution error: ${e}`);
                console.error(e);
            }
        };

        updatePerformance();
        animationFrameRef.current = requestAnimationFrame(frame);
    };

    const stopExecution = (): void => {
        if (!isRunningRef.current) {
            return;
        }

        isRunningRef.current = false;
        setIsRunning(false);
        if (animationFrameRef.current) {
            cancelAnimationFrame(animationFrameRef.current);
            animationFrameRef.current = null;
        }
        setStatus('Stopped');
    };

    const bootAndStart = (): void => {
        if (!computer || isRunningRef.current) {
            return;
        }

        try {
            // Boot from the selected drive
            computer.boot(bootDrive);
            const driveName = bootDrive === 0x00 ? 'floppy A:' : 'hard drive C:';
            setStatus(`Booted from ${driveName}, starting execution...`);

            // Start execution
            isRunningRef.current = true;
            setIsRunning(true);

            const frame = (): void => {
                if (!isRunningRef.current) {
                    return;
                }

                try {
                    pollGamepads(computer);
                    const stillRunning = computer.run_for_ms(16, window.performance.now());
                    updatePerformance();

                    if (!stillRunning) {
                        isRunningRef.current = false;
                        setIsRunning(false);
                        setStatus('CPU halted');
                        return;
                    }
                    animationFrameRef.current = requestAnimationFrame(frame);
                } catch (e) {
                    isRunningRef.current = false;
                    setIsRunning(false);
                    setStatus(`Execution error: ${e}`);
                    console.error(e);
                }
            };

            updatePerformance();
            animationFrameRef.current = requestAnimationFrame(frame);
        } catch (e) {
            setStatus(`Boot error: ${e}`);
            console.error(e);
        }
    };

    const loadProgram = (data: Uint8Array, segment: number, offset: number): void => {
        if (!computer) {
            return;
        }

        try {
            computer.load_program(data, segment, offset);
            setStatus(
                `Loaded program: ${data.length} bytes at ${segment.toString(16).toUpperCase().padStart(4, '0')}:${offset.toString(16).toUpperCase().padStart(4, '0')}`
            );
        } catch (e) {
            setStatus(`Load error: ${e}`);
            console.error(e);
        }
    };

    const reset = (): void => {
        if (!computer) {
            return;
        }

        try {
            computer.reset();
            stopExecution();
            setStatus('Computer reset');
        } catch (e) {
            setStatus(`Reset error: ${e}`);
            console.error(e);
        }
    };

    const applyConfig = useCallback(
        (config: EmulatorConfig) => {
            // Stop execution before recreating
            isRunningRef.current = false;
            setIsRunning(false);
            if (animationFrameRef.current) {
                cancelAnimationFrame(animationFrameRef.current);
                animationFrameRef.current = null;
            }

            // Clear gamepad assignments; they'll be re-assigned via the gamepad effect
            gamepadSlotsRef.current.clear();
            configRef.current = config;

            try {
                const comp = createComputer(config);
                setComputer(comp);
                setStatus(`Configuration applied: ${config.cpuType}, ${config.memoryKb}KB, ${config.clockMhz} MHz`);
            } catch (e) {
                setStatus(`Configuration error: ${e}`);
                console.error(e);
            }
        },
        [createComputer]
    );

    return {
        computer,
        status,
        setStatus,
        isRunning,
        performance,
        startExecution,
        stopExecution,
        loadProgram,
        reset,
        bootAndStart,
        applyConfig,
    };
}

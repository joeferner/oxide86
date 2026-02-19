import { signal } from '@preact/signals-react';
import wasmInit, { Emu86Computer } from '../pkg/emu86_wasm';
import { EmulatorConfig, loadConfig } from './components/ConfigDialog.consts';

export interface PerformanceStats {
    target: number;
    actual: number;
}

// Module-level signals — always current, no stale closures
export const computer = signal<Emu86Computer | null>(null);
export const status = signal('Initializing...');
export const isRunning = signal(false);
export const perfStats = signal<PerformanceStats>({ target: 0, actual: 0 });
export const joystickConnected = signal<[boolean, boolean]>([false, false]);
export const config = signal<EmulatorConfig>(loadConfig());
export const bootDrive = signal<number>(0x80);

// Imperative handles (not reactive state)
const animationFrameRef = { current: null as number | null };
const wasmInitializedRef = { current: false };
// Maps physical gamepad index → emulator joystick slot (0=A, 1=B)
const gamepadSlotsRef = { current: new Map<number, number>() };
// AdLib Web Audio state
const adlibAudioRef = { current: null as { context: AudioContext; node: AudioWorkletNode } | null };

function createComputer(cfg: EmulatorConfig): Emu86Computer {
    return new Emu86Computer({
        canvas_id: 'display',
        cpu_type: cfg.cpuType,
        memory_kb: cfg.memoryKb,
        clock_mhz: cfg.clockMhz,
        video_card: cfg.videoCard,
        com1_device: cfg.com1Device,
        com2_device: cfg.com2Device,
        audio_enabled: cfg.audioEnabled,
        sound_card: cfg.soundCard,
    });
}

// AudioWorklet processor as an inline Blob — no separate file or bundler config needed.
// The worklet maintains a Float32Array ring buffer fed via MessagePort from the main thread.
const ADLIB_WORKLET_CODE = `
class AdlibProcessor extends AudioWorkletProcessor {
    constructor() {
        super();
        this._buf = new Float32Array(0);
        this._pos = 0;
        this.port.onmessage = (e) => {
            const incoming = e.data;
            const remaining = this._buf.length - this._pos;
            const merged = new Float32Array(remaining + incoming.length);
            merged.set(this._buf.subarray(this._pos));
            merged.set(incoming, remaining);
            this._buf = merged;
            this._pos = 0;
        };
    }
    process(_inputs, outputs) {
        const out = outputs[0][0];
        if (!out) return true;
        const avail = this._buf.length - this._pos;
        const n = Math.min(out.length, avail);
        out.set(this._buf.subarray(this._pos, this._pos + n));
        this._pos += n;
        for (let i = n; i < out.length; i++) out[i] = 0;
        return true;
    }
}
registerProcessor('adlib-processor', AdlibProcessor);
`;

async function setupAdlibAudio(comp: Emu86Computer): Promise<void> {
    teardownAdlibAudio();
    try {
        const sampleRate = comp.enable_adlib();
        const context = new AudioContext({ sampleRate });
        if (!context.audioWorklet) {
            console.warn('[adlib] AudioWorklet not available (requires HTTPS or localhost); AdLib audio disabled');
            void context.close();
            return;
        }
        const blob = new Blob([ADLIB_WORKLET_CODE], { type: 'application/javascript' });
        const url = URL.createObjectURL(blob);
        await context.audioWorklet.addModule(url);
        URL.revokeObjectURL(url);
        const node = new AudioWorkletNode(context, 'adlib-processor');
        node.connect(context.destination);
        adlibAudioRef.current = { context, node };
        console.log(`[adlib] AudioWorklet initialized: ${sampleRate} Hz`);
    } catch (err) {
        console.error('[adlib] Failed to initialize AudioWorklet:', err);
    }
}

function teardownAdlibAudio(): void {
    if (adlibAudioRef.current) {
        adlibAudioRef.current.node.disconnect();
        void adlibAudioRef.current.context.close();
        adlibAudioRef.current = null;
    }
}

function setJoystickConnectedSlot(comp: Emu86Computer | null | undefined, slot: number, connected: boolean): void {
    const label = slot === 0 ? 'A' : 'B';
    console.log(`[joystick] Joystick ${label} (slot ${slot}): ${connected ? 'connected' : 'disconnected'}`);
    comp?.gamepad_connected(slot, connected);
    const prev = joystickConnected.value;
    const next: [boolean, boolean] = [prev[0], prev[1]];
    next[slot] = connected;
    joystickConnected.value = next;
}

function assignGamepad(gamepadIndex: number): void {
    const cfg = config.value;
    const assigned = new Set(gamepadSlotsRef.current.values());
    console.log(
        `[joystick] Physical gamepad ${gamepadIndex} connected. joystickA=${cfg.joystickA}, joystickB=${cfg.joystickB}, assigned slots=${JSON.stringify([...assigned])}`
    );
    if (cfg.joystickA && !assigned.has(0)) {
        gamepadSlotsRef.current.set(gamepadIndex, 0);
        setJoystickConnectedSlot(computer.value, 0, true);
    } else if (cfg.joystickB && !assigned.has(1)) {
        gamepadSlotsRef.current.set(gamepadIndex, 1);
        setJoystickConnectedSlot(computer.value, 1, true);
    } else {
        console.log(`[joystick] Gamepad ${gamepadIndex} not assigned: no enabled slots available`);
    }
}

function pollGamepads(comp: Emu86Computer): void {
    const cfg = config.value;
    if (!cfg.joystickA && !cfg.joystickB) {
        return;
    }
    const gamepads = navigator.getGamepads();

    // Self-healing: assign any newly-visible gamepads missed by gamepadconnected event
    for (const gp of gamepads) {
        if (gp && !gamepadSlotsRef.current.has(gp.index)) {
            assignGamepad(gp.index);
        }
    }

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
            } catch (err) {
                console.error('[joystick] report failed', err);
            }
        }
    }
}

function updatePerformance(): void {
    const comp = computer.value;
    if (comp) {
        try {
            perfStats.value = { target: comp.get_target_mhz(), actual: comp.get_actual_mhz() };
        } catch {
            // Silently fail if not ready
        }
    }
}

function runLoop(): void {
    if (!isRunning.value) {
        return;
    }
    const comp = computer.value;
    if (!comp) {
        return;
    }

    try {
        pollGamepads(comp);
        const stillRunning = comp.run_for_ms(16, window.performance.now());
        updatePerformance();

        // Push AdLib samples to AudioWorklet.
        // Request only one frame's worth of audio + small margin.
        // Requesting too many (e.g. 2048) pads with zeros and creates ~30 ms
        // silence gaps every frame, producing an audible repeating/stuttering effect.
        if (adlibAudioRef.current) {
            const { context, node } = adlibAudioRef.current;
            // 16 ms frame * sampleRate / 1000, plus 64-sample margin for timing variance
            const frameSize = Math.ceil((context.sampleRate * 16) / 1000) + 64;
            const samples = comp.get_adlib_samples(frameSize);
            node.port.postMessage(samples, [samples.buffer]);
        }

        if (!stillRunning) {
            isRunning.value = false;
            status.value = 'CPU halted';
            return;
        }
        animationFrameRef.current = requestAnimationFrame(runLoop);
    } catch (e) {
        isRunning.value = false;
        status.value = `Execution error: ${e}`;
        console.error(e);
    }
}

function setupGamepadListeners(): void {
    window.addEventListener('gamepadconnected', (e: GamepadEvent) => {
        assignGamepad(e.gamepad.index);
    });
    window.addEventListener('gamepaddisconnected', (e: GamepadEvent) => {
        const slot = gamepadSlotsRef.current.get(e.gamepad.index);
        if (slot !== undefined) {
            console.log(`[joystick] Physical gamepad ${e.gamepad.index} disconnected from slot ${slot}`);
            setJoystickConnectedSlot(computer.value, slot, false);
            gamepadSlotsRef.current.delete(e.gamepad.index);
        }
    });
}

export async function initEmulator(canvasEl: HTMLCanvasElement): Promise<void> {
    // canvasEl guards that the canvas exists; WASM uses the canvas_id
    void canvasEl;
    try {
        if (!wasmInitializedRef.current) {
            await wasmInit();
            wasmInitializedRef.current = true;
            setupGamepadListeners();
        }

        const cfg = config.value;
        const comp = createComputer(cfg);
        console.log(`[joystick] Init: joystickA=${cfg.joystickA}, joystickB=${cfg.joystickB}`);

        // Scan physical gamepads already connected
        const gamepads = navigator.getGamepads();
        console.log(`[joystick] Physical gamepads detected: ${gamepads.filter(Boolean).length}`);
        for (const gp of gamepads) {
            if (gp && !gamepadSlotsRef.current.has(gp.index)) {
                assignGamepad(gp.index);
            }
        }

        // Set up AdLib audio if configured
        if (cfg.soundCard === 'adlib') {
            void setupAdlibAudio(comp);
        }

        computer.value = comp;
        status.value = 'Emulator initialized. Load disk images to begin.';
    } catch (e) {
        status.value = `Initialization error: ${e}`;
        console.error(e);
    }
}

export function startExecution(): void {
    if (!computer.value || isRunning.value) {
        return;
    }
    isRunning.value = true;
    status.value = 'Running... (click on display for keyboard input)';
    updatePerformance();
    animationFrameRef.current = requestAnimationFrame(runLoop);
}

export function stopExecution(): void {
    if (!isRunning.value) {
        return;
    }
    isRunning.value = false;
    if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
    }
    status.value = 'Stopped';
}

export function bootAndStart(): void {
    const comp = computer.value;
    if (!comp || isRunning.value) {
        return;
    }
    try {
        comp.boot(bootDrive.value);
        const driveName = bootDrive.value === 0x00 ? 'floppy A:' : 'hard drive C:';
        status.value = `Booted from ${driveName}, starting execution...`;
        isRunning.value = true;
        updatePerformance();
        animationFrameRef.current = requestAnimationFrame(runLoop);
    } catch (e) {
        status.value = `Boot error: ${e}`;
        console.error(e);
    }
}

export function loadProgram(data: Uint8Array, segment: number, offset: number): void {
    const comp = computer.value;
    if (!comp) {
        return;
    }
    try {
        comp.load_program(data, segment, offset);
        status.value = `Loaded program: ${data.length} bytes at ${segment.toString(16).toUpperCase().padStart(4, '0')}:${offset.toString(16).toUpperCase().padStart(4, '0')}`;
    } catch (e) {
        status.value = `Load error: ${e}`;
        console.error(e);
    }
}

export function reset(): void {
    const comp = computer.value;
    if (!comp) {
        return;
    }
    try {
        comp.reset();
        stopExecution();
        status.value = 'Computer reset';
    } catch (e) {
        status.value = `Reset error: ${e}`;
        console.error(e);
    }
}

export function applyConfig(cfg: EmulatorConfig): void {
    // Stop execution before recreating
    isRunning.value = false;
    if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
    }

    // Tear down AdLib audio before recreating
    teardownAdlibAudio();

    // Clear gamepad assignments and connection state; they'll be re-assigned below
    gamepadSlotsRef.current.clear();
    joystickConnected.value = [false, false];
    config.value = cfg;

    try {
        const comp = createComputer(cfg);
        console.log(`[joystick] applyConfig: joystickA=${cfg.joystickA}, joystickB=${cfg.joystickB}`);

        // Re-scan physical gamepads and assign them
        const gamepads = navigator.getGamepads();
        for (const gp of gamepads) {
            if (gp && !gamepadSlotsRef.current.has(gp.index)) {
                assignGamepad(gp.index);
            }
        }

        // Set up AdLib audio if configured
        if (cfg.soundCard === 'adlib') {
            void setupAdlibAudio(comp);
        }

        computer.value = comp;
        status.value = `Configuration applied: ${cfg.cpuType}, ${cfg.memoryKb}KB, ${cfg.clockMhz} MHz`;
    } catch (e) {
        status.value = `Configuration error: ${e}`;
        console.error(e);
    }
}

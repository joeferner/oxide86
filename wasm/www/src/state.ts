import { signal, type ReadonlySignal } from '@preact/signals-react';
import { Oxide86Computer, type WasmComputerConfig } from 'oxide86-wasm';

const CONFIG_STORAGE_KEY = 'oxide86_machine_config';
const COM_PORTS_STORAGE_KEY = 'oxide86_com_ports';
const LPT_PORTS_STORAGE_KEY = 'oxide86_lpt_ports';

export type ComPortDevice = 'none' | 'serial_mouse' | 'loopback';
export const COM_PORT_COUNT = 4;

export type LptPortDevice = 'none' | 'printer' | 'loopback';
export const LPT_PORT_COUNT = 3;

function loadComPorts(): ComPortDevice[] {
    try {
        const raw = localStorage.getItem(COM_PORTS_STORAGE_KEY);
        if (raw) {
            const parsed = JSON.parse(raw) as unknown[];
            if (Array.isArray(parsed)) {
                return Array.from({ length: COM_PORT_COUNT }, (_, i) => (parsed[i] as ComPortDevice) ?? 'none');
            }
        }
    } catch {
        // ignore
    }
    return Array(COM_PORT_COUNT).fill('none') as ComPortDevice[];
}

function saveComPorts(ports: ComPortDevice[]): void {
    localStorage.setItem(COM_PORTS_STORAGE_KEY, JSON.stringify(ports));
}

function loadLptPorts(): LptPortDevice[] {
    try {
        const raw = localStorage.getItem(LPT_PORTS_STORAGE_KEY);
        if (raw) {
            const parsed = JSON.parse(raw) as unknown[];
            if (Array.isArray(parsed)) {
                return Array.from({ length: LPT_PORT_COUNT }, (_, i) => (parsed[i] as LptPortDevice) ?? 'none');
            }
        }
    } catch {
        // ignore
    }
    return Array(LPT_PORT_COUNT).fill('none') as LptPortDevice[];
}

function saveLptPorts(ports: LptPortDevice[]): void {
    localStorage.setItem(LPT_PORTS_STORAGE_KEY, JSON.stringify(ports));
}

type PersistedConfig = Pick<
    WasmComputerConfig,
    'cpu_type' | 'has_fpu' | 'memory_kb' | 'clock_hz' | 'video_card' | 'sound_card'
>;

function loadPersistedConfig(): Partial<PersistedConfig> {
    try {
        const raw = localStorage.getItem(CONFIG_STORAGE_KEY);
        if (raw) {
            return JSON.parse(raw) as Partial<PersistedConfig>;
        }
    } catch {
        // ignore parse errors
    }
    return {};
}

function saveConfig(config: WasmComputerConfig): void {
    const persisted: PersistedConfig = {
        cpu_type: config.cpu_type,
        has_fpu: config.has_fpu,
        memory_kb: config.memory_kb,
        clock_hz: config.clock_hz,
        video_card: config.video_card,
        sound_card: config.sound_card,
    };
    localStorage.setItem(CONFIG_STORAGE_KEY, JSON.stringify(persisted));
}

const HARDWARE_DEFAULTS: PersistedConfig = {
    cpu_type: '286',
    has_fpu: false,
    memory_kb: 1024,
    clock_hz: 6_000_000,
    video_card: 'vga',
    sound_card: 'adlib',
};

function defaultConfig(): WasmComputerConfig {
    const now = new Date();
    const saved = loadPersistedConfig();
    return {
        ...HARDWARE_DEFAULTS,
        ...saved,
        start_year: now.getFullYear(),
        start_month: now.getMonth() + 1,
        start_day: now.getDate(),
        start_hour: now.getHours(),
        start_minute: now.getMinutes(),
        start_second: now.getSeconds(),
    };
}

interface SoundAudio {
    context: AudioContext;
    node: AudioWorkletNode;
}

export class State {
    private readonly computerSignal = signal<Oxide86Computer | null>(null);
    private readonly powerStateSignal = signal<'on' | 'off' | 'warning'>('off');
    private readonly errorSignal = signal<string | null>(null);
    private readonly configSignal = signal<WasmComputerConfig>(defaultConfig());
    private readonly perfSignal = signal<number>(0); // MHz
    private readonly floppyASignal = signal<File | null>(null);
    private readonly floppyBSignal = signal<File | null>(null);
    private readonly hddSignal = signal<File | null>(null);
    private readonly bootDriveSignal = signal<0 | 1 | 'hdd' | null>('hdd');
    private readonly comPortsSignal = signal<ComPortDevice[]>(loadComPorts());
    private readonly lptPortsSignal = signal<LptPortDevice[]>(loadLptPorts());
    private soundAudio: SoundAudio | null = null;

    // ── Read-only signal accessors ────────────────────────────────────────────

    public get computer(): ReadonlySignal<Oxide86Computer | null> {
        return this.computerSignal;
    }

    public get powerState(): ReadonlySignal<'on' | 'off' | 'warning'> {
        return this.powerStateSignal;
    }

    public get error(): ReadonlySignal<string | null> {
        return this.errorSignal;
    }

    public get config(): ReadonlySignal<WasmComputerConfig> {
        return this.configSignal;
    }

    public get floppyA(): ReadonlySignal<File | null> {
        return this.floppyASignal;
    }

    public get floppyB(): ReadonlySignal<File | null> {
        return this.floppyBSignal;
    }

    public get hdd(): ReadonlySignal<File | null> {
        return this.hddSignal;
    }

    public get comPorts(): ReadonlySignal<ComPortDevice[]> {
        return this.comPortsSignal;
    }

    public get lptPorts(): ReadonlySignal<LptPortDevice[]> {
        return this.lptPortsSignal;
    }

    public get bootDrive(): ReadonlySignal<0 | 1 | 'hdd' | null> {
        return this.bootDriveSignal;
    }

    // ── Config ────────────────────────────────────────────────────────────────

    public updateConfig(patch: Partial<WasmComputerConfig>): void {
        const next = { ...this.configSignal.value, ...patch };
        this.configSignal.value = next;
        saveConfig(next);
    }

    public resetConfig(): void {
        this.updateConfig(HARDWARE_DEFAULTS);
    }

    // ── Status ────────────────────────────────────────────────────────────────

    // Called by Screen when the emulator halts.
    public setStatus(powerState: 'on' | 'off' | 'warning', error?: string | null): void {
        this.powerStateSignal.value = powerState;
        this.errorSignal.value = error ?? null;
    }

    public dismissError(): void {
        this.errorSignal.value = null;
    }

    public get perf(): ReadonlySignal<number> {
        return this.perfSignal;
    }

    public sampleMhz(): void {
        const mhz = this.computerSignal.value?.get_effective_mhz() ?? 0;
        this.perfSignal.value = mhz;
    }

    // ── Sound card audio ──────────────────────────────────────────────────────

    public async setupSoundCardAudio(sampleRate: number): Promise<void> {
        this.teardownSoundCardAudio();
        try {
            const context = new AudioContext({ sampleRate });
            if (!context.audioWorklet) {
                console.warn('[Sound Card] AudioWorklet not available (requires HTTPS or localhost)');
                void context.close();
                return;
            }
            const workletUrl = new URL('./soundCardWorklet.ts', import.meta.url);
            await context.audioWorklet.addModule(workletUrl);
            const node = new AudioWorkletNode(context, 'sound-card-processor');
            node.connect(context.destination);
            node.port.onmessage = (
                e: MessageEvent<{
                    type: string;
                    underrunCount: number;
                    nonzeroCount: number;
                    totalCount: number;
                    backlog: number;
                }>
            ) => {
                if (e.data?.type !== 'stats') {
                    return;
                }
                const { underrunCount, nonzeroCount, totalCount, backlog } = e.data;
                if (underrunCount > 0) {
                    console.debug(`[Sound Card] Underrun: ${underrunCount} silence samples in last ~1s`);
                }
                if (backlog > sampleRate / 2) {
                    console.warn(
                        `[Sound Card] Overrun: ${backlog} samples backlogged (~${((backlog / sampleRate) * 1000).toFixed(0)} ms)`
                    );
                }
                console.debug(
                    `[Sound Card] PCM: ${nonzeroCount}/${totalCount} non-zero (${((100 * nonzeroCount) / totalCount).toFixed(1)}%), backlog: ${backlog}`
                );
            };
            this.soundAudio = { context, node };
            console.log(`[Sound Card] AudioWorklet initialized: ${sampleRate} Hz`);
        } catch (err) {
            console.error('[Sound Card] Failed to initialize AudioWorklet:', err);
        }
    }

    public teardownSoundCardAudio(): void {
        if (this.soundAudio) {
            this.soundAudio.node.disconnect();
            void this.soundAudio.context.close();
            this.soundAudio = null;
        }
    }

    public resumeAudio(): void {
        if (this.soundAudio?.context.state === 'suspended') {
            void this.soundAudio.context.resume();
        }
    }

    public feedSoundCardSamples(computer: Oxide86Computer, elapsedMs: number): void {
        if (!this.soundAudio) {
            return;
        }
        const { context, node } = this.soundAudio;
        const frameSize = Math.ceil((context.sampleRate * elapsedMs) / 1000) + 64;
        const samples = computer.get_sound_card_samples(frameSize);
        node.port.postMessage({ type: 'samples', samples }, [samples.buffer]);
    }

    // ── Power ─────────────────────────────────────────────────────────────────

    public async powerOn(): Promise<void> {
        if (!this.floppyASignal.value && !this.hddSignal.value) {
            this.powerStateSignal.value = 'off';
            this.errorSignal.value = 'Load a floppy or hard disk image first';
            return;
        }

        let computer: Oxide86Computer;
        try {
            computer = new Oxide86Computer(this.configSignal.value);
        } catch (e) {
            this.powerStateSignal.value = 'warning';
            this.errorSignal.value = String(e);
            return;
        }

        const toBytes = async (f: File | null): Promise<Uint8Array | null> =>
            f ? new Uint8Array(await f.arrayBuffer()) : null;

        const [hddImage, floppyAImage, floppyBImage] = await Promise.all([
            toBytes(this.hddSignal.value),
            toBytes(this.floppyASignal.value),
            toBytes(this.floppyBSignal.value),
        ]);

        const bootDriveMap: Record<string, string> = { '0': 'floppy_a', '1': 'floppy_b', hdd: 'hdd' };
        const bootDrive =
            this.bootDriveSignal.value != null ? bootDriveMap[String(this.bootDriveSignal.value)] : undefined;

        // Push COM port config into the new instance before boot so start_computer picks them up.
        this.comPortsSignal.value.forEach((device, i) => {
            if (device && device !== 'none') {
                computer.set_com_port_device((i + 1) as 1 | 2 | 3 | 4, device);
            }
        });

        // Push LPT port config into the new instance before boot.
        this.lptPortsSignal.value.forEach((device, i) => {
            if (device && device !== 'none') {
                computer.set_lpt_port_device((i + 1) as 1 | 2 | 3, device);
            }
        });

        computer.power_on(hddImage, floppyAImage, floppyBImage, bootDrive);

        if (this.configSignal.value.sound_card === 'adlib') {
            void this.setupSoundCardAudio(computer.get_sound_card_sample_rate());
        }

        this.computerSignal.value = computer;
        this.powerStateSignal.value = 'on';
        this.errorSignal.value = null;
    }

    public powerOff(): void {
        const computer = this.computerSignal.value;
        if (!computer) {
            return;
        }
        this.teardownSoundCardAudio();
        computer.power_off();
        this.computerSignal.value = null;
        this.powerStateSignal.value = 'off';
        this.errorSignal.value = null;
    }

    public reboot(): void {
        // The RAF loop in Screen keeps running — reboot resets internal state
        // on the existing computer object; no need to re-trigger the effect.
        this.teardownSoundCardAudio();
        this.computerSignal.value?.reboot();

        const computer = this.computerSignal.value;
        if (computer && this.configSignal.value.sound_card === 'adlib') {
            void this.setupSoundCardAudio(computer.get_sound_card_sample_rate());
        }

        this.powerStateSignal.value = 'on';
        this.errorSignal.value = null;
    }

    // ── Drives ────────────────────────────────────────────────────────────────

    public async insertFloppy(drive: 0 | 1, file: File): Promise<void> {
        const image = new Uint8Array(await file.arrayBuffer());
        this.computerSignal.value?.insert_floppy(drive, image);
        if (drive === 0) {
            this.floppyASignal.value = file;
        } else {
            this.floppyBSignal.value = file;
        }
    }

    public ejectFloppy(drive: 0 | 1): void {
        this.computerSignal.value?.eject_floppy(drive);
        if (drive === 0) {
            this.floppyASignal.value = null;
        } else {
            this.floppyBSignal.value = null;
        }
    }

    public setHdd(file: File | null): void {
        this.hddSignal.value = file;
    }

    public setBootDrive(drive: 0 | 1 | 'hdd' | null): void {
        this.bootDriveSignal.value = drive;
    }

    // ── COM ports ─────────────────────────────────────────────────────────────

    public setComPortDevice(port: 1 | 2 | 3 | 4, device: ComPortDevice): void {
        const next = [...this.comPortsSignal.value] as ComPortDevice[];
        next[port - 1] = device;
        this.comPortsSignal.value = next;
        saveComPorts(next);
        const computer = this.computerSignal.value;
        if (computer) {
            computer.set_com_port_device(port, device);
        }
    }

    public setLptPortDevice(port: 1 | 2 | 3, device: LptPortDevice): void {
        const next = [...this.lptPortsSignal.value] as LptPortDevice[];
        next[port - 1] = device;
        this.lptPortsSignal.value = next;
        saveLptPorts(next);
        const computer = this.computerSignal.value;
        if (computer) {
            computer.set_lpt_port_device(port, device);
        }
    }
}

export const state = new State();

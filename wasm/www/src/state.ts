import { signal, type ReadonlySignal } from '@preact/signals-react';
import { Oxide86Computer, type WasmComputerConfig } from 'oxide86-wasm';

const CONFIG_STORAGE_KEY = 'oxide86_machine_config';

type PersistedConfig = Pick<WasmComputerConfig, 'cpu_type' | 'has_fpu' | 'memory_kb' | 'clock_hz' | 'video_card'>;

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
    };
    localStorage.setItem(CONFIG_STORAGE_KEY, JSON.stringify(persisted));
}

const HARDWARE_DEFAULTS: PersistedConfig = {
    cpu_type: '286',
    has_fpu: false,
    memory_kb: 1024,
    clock_hz: 6_000_000,
    video_card: 'vga',
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

        computer.power_on(hddImage, floppyAImage, floppyBImage, bootDrive);
        this.computerSignal.value = computer;
        this.powerStateSignal.value = 'on';
        this.errorSignal.value = null;
    }

    public powerOff(): void {
        const computer = this.computerSignal.value;
        if (!computer) {
            return;
        }
        computer.power_off();
        this.computerSignal.value = null;
        this.powerStateSignal.value = 'off';
        this.errorSignal.value = null;
    }

    public reboot(): void {
        // The RAF loop in Screen keeps running — reboot resets internal state
        // on the existing computer object; no need to re-trigger the effect.
        this.computerSignal.value?.reboot();
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
}

export const state = new State();

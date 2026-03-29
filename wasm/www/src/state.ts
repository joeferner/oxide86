import { signal, type ReadonlySignal } from '@preact/signals-react';
import { Oxide86Computer, type WasmComputerConfig } from 'oxide86-wasm';

function defaultConfig(): WasmComputerConfig {
    const now = new Date();
    return {
        cpu_type: '286',
        has_fpu: false,
        memory_kb: 1024,
        clock_hz: 6_000_000,
        video_card: 'vga',
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

    // ── Config ────────────────────────────────────────────────────────────────

    public updateConfig(patch: Partial<WasmComputerConfig>): void {
        this.configSignal.value = { ...this.configSignal.value, ...patch };
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

        const hddFile = this.hddSignal.value;
        const floppyFile = this.floppyASignal.value;
        const hddImage = hddFile ? new Uint8Array(await hddFile.arrayBuffer()) : null;
        const floppyImage = floppyFile ? new Uint8Array(await floppyFile.arrayBuffer()) : null;

        computer.power_on(hddImage, floppyImage);
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
}

export const state = new State();

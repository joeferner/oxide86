import { signal } from '@preact/signals-react';
import type { Oxide86Computer, WasmComputerConfig } from 'oxide86-wasm';

function defaultConfig(): WasmComputerConfig {
    const now = new Date();
    return {
        cpu_type: '8086',
        has_fpu: false,
        memory_kb: 640,
        clock_hz: 4_772_727,
        video_card: 'ega',
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
    private readonly statusSignal = signal<{ message: string; error: string | null }>({
        message: 'Off',
        error: null,
    });
    private readonly configSignal = signal<WasmComputerConfig>(defaultConfig());
    private readonly perfSignal = signal<number>(0); // MHz
    private readonly floppyASignal = signal<File | null>(null);
    private readonly floppyBSignal = signal<File | null>(null);
    private readonly hddSignal = signal<File | null>(null);
}

export const state = new State();

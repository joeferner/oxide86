export interface EmulatorConfig {
    cpuType: string;
    memoryKb: number;
    clockMhz: number;
    videoCard: string;
    com1Device: string;
    com2Device: string;
    joystickA: boolean;
    joystickB: boolean;
}

export const DEFAULT_CONFIG: EmulatorConfig = {
    cpuType: '8086',
    memoryKb: 640,
    clockMhz: 4.77,
    videoCard: 'ega',
    com1Device: 'mouse',
    com2Device: 'null',
    joystickA: false,
    joystickB: false,
};

export const CPU_OPTIONS = [
    { value: '8086', label: '8086 (original, 1 MB)' },
    { value: '286', label: '286 (16 MB)' },
    { value: '386', label: '386 (64 MB)' },
    { value: '486', label: '486 (64 MB)' },
];

// Total memory options: conventional (≤640KB) or extended (>640KB, requires 286+)
export const MEMORY_OPTIONS = [
    { value: '256', label: '256 KB (conventional)' },
    { value: '512', label: '512 KB (conventional)' },
    { value: '640', label: '640 KB (conventional, standard)' },
    { value: '1024', label: '1 MB (no extended)' },
    { value: '2048', label: '2 MB (1 MB extended, 286+)' },
    { value: '4096', label: '4 MB (3 MB extended, 286+)' },
    { value: '8192', label: '8 MB (7 MB extended, 286+)' },
    { value: '16384', label: '16 MB (15 MB extended, 286+)' },
];

export const CLOCK_OPTIONS = [
    { value: '4.77', label: '4.77 MHz (IBM PC/XT, 8088)' },
    { value: '8', label: '8 MHz (IBM PC/AT, 286)' },
    { value: '10', label: '10 MHz (PC/AT 10 MHz, 286)' },
    { value: '12', label: '12 MHz (286)' },
    { value: '16', label: '16 MHz (386SX)' },
    { value: '25', label: '25 MHz (386DX / 486SX)' },
    { value: '33', label: '33 MHz (486DX)' },
    { value: '100', label: '100 MHz (Pentium)' },
];

export const VIDEO_CARD_OPTIONS = [
    { value: 'cga', label: 'CGA (text + 4-color graphics)' },
    { value: 'ega', label: 'EGA (CGA + 16-color graphics)' },
    { value: 'vga', label: 'VGA (EGA + VGA modes)' },
];

export const COM_PORT_OPTIONS = [
    { value: 'null', label: 'None' },
    { value: 'mouse', label: 'Serial Mouse' },
];

const CONFIG_STORAGE_KEY = 'emu86_config';

export function loadConfig(): EmulatorConfig {
    try {
        const stored = localStorage.getItem(CONFIG_STORAGE_KEY);
        if (stored) {
            const parsed = JSON.parse(stored) as Partial<EmulatorConfig>;
            return { ...DEFAULT_CONFIG, ...parsed };
        }
    } catch {
        // ignore
    }
    return DEFAULT_CONFIG;
}

export function saveConfig(config: EmulatorConfig): void {
    try {
        localStorage.setItem(CONFIG_STORAGE_KEY, JSON.stringify(config));
    } catch {
        // ignore
    }
}

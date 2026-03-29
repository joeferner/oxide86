import React, { useState } from 'react';
import { Select, Switch, NumberInput, Stack, Text, Button, Group } from '@mantine/core';
import { useSignalEffect } from '@preact/signals-react';
import { state } from '../state';
import type { WasmComputerConfig } from 'oxide86-wasm';
import styles from './Toolbar.module.scss';

const CLOCK_OPTIONS = [
    { value: '4772727', label: '4.77 MHz (XT)' },
    { value: '6000000', label: '6 MHz (stock 286)' },
    { value: '8000000', label: '8 MHz' },
    { value: '10000000', label: '10 MHz' },
    { value: 'custom', label: 'Custom' },
];

const KNOWN_CLOCKS = new Set([4_772_727, 6_000_000, 8_000_000, 10_000_000]);

export function MachineConfig(): React.ReactElement {
    const [config, setConfig] = useState<WasmComputerConfig>(state.config.value);
    const [running, setRunning] = useState(state.computer.value !== null);
    const [clockOption, setClockOption] = useState<string>(() =>
        KNOWN_CLOCKS.has(state.config.value.clock_hz) ? String(state.config.value.clock_hz) : 'custom'
    );
    const [confirmReset, setConfirmReset] = useState(false);

    useSignalEffect(() => {
        const cfg = state.config.value;
        setConfig(cfg);
        setClockOption(KNOWN_CLOCKS.has(cfg.clock_hz) ? String(cfg.clock_hz) : 'custom');
    });

    useSignalEffect(() => {
        setRunning(state.computer.value !== null);
    });

    const disabled = running;

    const patch = (p: Partial<WasmComputerConfig>): void => {
        state.updateConfig(p);
    };

    const onClockOptionChange = (val: string | null): void => {
        if (!val) {
            return;
        }
        setClockOption(val);
        if (val !== 'custom') {
            patch({ clock_hz: Number(val) });
        }
    };

    return (
        <Stack gap="md" className={styles.panel}>
            <Text size="sm" fw={600}>
                Machine
            </Text>
            {disabled && (
                <Text size="xs" c="dimmed">
                    Power off to change settings.
                </Text>
            )}
            <Select
                label="CPU"
                data={[
                    { value: '8086', label: 'Intel 8086' },
                    { value: '286', label: 'Intel 286' },
                ]}
                value={config.cpu_type}
                onChange={(v) => {
                    if (v) {
                        patch({ cpu_type: v });
                    }
                }}
                disabled={disabled}
            />

            <Switch
                label="Math coprocessor (FPU)"
                checked={config.has_fpu}
                onChange={(e) => {
                    patch({ has_fpu: e.currentTarget.checked });
                }}
                disabled={disabled}
            />

            <NumberInput
                label="RAM (KB)"
                value={config.memory_kb}
                min={64}
                max={65536}
                step={64}
                onChange={(v) => {
                    if (typeof v === 'number') {
                        patch({ memory_kb: v });
                    }
                }}
                disabled={disabled}
            />

            <Select
                label="Clock speed"
                data={CLOCK_OPTIONS}
                value={clockOption}
                onChange={onClockOptionChange}
                disabled={disabled}
            />

            {clockOption === 'custom' && (
                <NumberInput
                    label="Custom clock (Hz)"
                    value={config.clock_hz}
                    min={1_000_000}
                    max={100_000_000}
                    step={1_000_000}
                    onChange={(v) => {
                        if (typeof v === 'number') {
                            patch({ clock_hz: v });
                        }
                    }}
                    disabled={disabled}
                />
            )}

            <Select
                label="Video card"
                data={[
                    { value: 'cga', label: 'CGA' },
                    { value: 'ega', label: 'EGA' },
                    { value: 'vga', label: 'VGA' },
                ]}
                value={config.video_card}
                onChange={(v) => {
                    if (v) {
                        patch({ video_card: v });
                    }
                }}
                disabled={disabled}
            />

            {confirmReset ? (
                <Stack gap="xs">
                    <Text size="xs" c="dimmed">
                        Reset all settings to defaults?
                    </Text>
                    <Group gap="xs">
                        <Button
                            size="xs"
                            color="red"
                            onClick={() => {
                                state.resetConfig();
                                setConfirmReset(false);
                            }}
                        >
                            Reset
                        </Button>
                        <Button size="xs" variant="subtle" onClick={() => { setConfirmReset(false); }}>
                            Cancel
                        </Button>
                    </Group>
                </Stack>
            ) : (
                <Button
                    size="xs"
                    variant="subtle"
                    color="red"
                    disabled={disabled}
                    onClick={() => { setConfirmReset(true); }}
                >
                    Reset to defaults
                </Button>
            )}
        </Stack>
    );
}

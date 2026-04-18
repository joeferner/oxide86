import React, { useState } from 'react';
import { ActionIcon, Group, Select, Stack, Text, Tooltip } from '@mantine/core';
import { useSignalEffect } from '@preact/signals-react';
import { state, type ComPortDevice, COM_PORT_COUNT, type LptPortDevice, LPT_PORT_COUNT } from '../state';
import styles from './Toolbar.module.scss';

const COM_DEVICE_OPTIONS = [
    { value: 'none', label: 'None' },
    { value: 'serial_mouse', label: 'Serial mouse' },
    { value: 'loopback', label: 'Loopback' },
];

const LPT_DEVICE_OPTIONS = [
    { value: 'none', label: 'None' },
    { value: 'printer', label: 'Printer' },
    { value: 'loopback', label: 'Loopback' },
];

function downloadPrinterOutput(port: 1 | 2 | 3): void {
    const computer = state.computer.peek();
    if (!computer) {
        return;
    }
    const data = computer.get_lpt_output(port);
    if (data.length === 0) {
        return;
    }
    const blob = new Blob([data.slice()], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `lpt${port}.prn`;
    a.click();
    URL.revokeObjectURL(url);
}

export function PeripheralsPanel(): React.ReactElement {
    const [comPorts, setComPorts] = useState<ComPortDevice[]>(state.comPorts.value);
    const [lptPorts, setLptPorts] = useState<LptPortDevice[]>(state.lptPorts.value);
    const [isRunning, setIsRunning] = useState(() => state.computer.peek() !== null);

    useSignalEffect(() => {
        setComPorts(state.comPorts.value);
    });

    useSignalEffect(() => {
        setLptPorts(state.lptPorts.value);
    });

    useSignalEffect(() => {
        setIsRunning(state.computer.value !== null);
    });

    return (
        <Stack gap="md" className={styles.panel}>
            <Text size="sm" fw={600}>
                Peripherals
            </Text>
            <Stack gap="md" className={styles.scroll}>
                {Array.from({ length: COM_PORT_COUNT }, (_, i) => {
                    const port = (i + 1) as 1 | 2 | 3 | 4;
                    return (
                        <Select
                            key={`com${port}`}
                            label={`COM${port}`}
                            data={COM_DEVICE_OPTIONS}
                            value={comPorts[i] ?? 'none'}
                            onChange={(v) => {
                                if (v) {
                                    state.setComPortDevice(port, v as ComPortDevice);
                                }
                            }}
                        />
                    );
                })}
                {Array.from({ length: LPT_PORT_COUNT }, (_, i) => {
                    const port = (i + 1) as 1 | 2 | 3;
                    const isPrinter = lptPorts[i] === 'printer';
                    return (
                        <Group key={`lpt${port}`} align="flex-end" gap="xs">
                            <Select
                                style={{ flex: 1 }}
                                label={`LPT${port}`}
                                data={LPT_DEVICE_OPTIONS}
                                value={lptPorts[i] ?? 'none'}
                                onChange={(v) => {
                                    if (v) {
                                        state.setLptPortDevice(port, v as LptPortDevice);
                                    }
                                }}
                            />
                            <Tooltip label={`Download LPT${port} output`} position="right">
                                <ActionIcon
                                    variant="default"
                                    size="lg"
                                    disabled={!isPrinter || !isRunning}
                                    onClick={() => {
                                        downloadPrinterOutput(port);
                                    }}
                                    aria-label={`Download LPT${port} output`}
                                >
                                    <i className="bi bi-download" />
                                </ActionIcon>
                            </Tooltip>
                        </Group>
                    );
                })}
            </Stack>
        </Stack>
    );
}

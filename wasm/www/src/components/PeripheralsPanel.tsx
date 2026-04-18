import React, { useState } from 'react';
import { Select, Stack, Text } from '@mantine/core';
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

export function PeripheralsPanel(): React.ReactElement {
    const [comPorts, setComPorts] = useState<ComPortDevice[]>(state.comPorts.value);
    const [lptPorts, setLptPorts] = useState<LptPortDevice[]>(state.lptPorts.value);

    useSignalEffect(() => {
        setComPorts(state.comPorts.value);
    });

    useSignalEffect(() => {
        setLptPorts(state.lptPorts.value);
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
                    return (
                        <Select
                            key={`lpt${port}`}
                            label={`LPT${port}`}
                            data={LPT_DEVICE_OPTIONS}
                            value={lptPorts[i] ?? 'none'}
                            onChange={(v) => {
                                if (v) {
                                    state.setLptPortDevice(port, v as LptPortDevice);
                                }
                            }}
                        />
                    );
                })}
            </Stack>
        </Stack>
    );
}

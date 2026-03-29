import React, { useState } from 'react';
import { Select, Stack, Text } from '@mantine/core';
import { useSignalEffect } from '@preact/signals-react';
import { state, type ComPortDevice, COM_PORT_COUNT } from '../state';
import styles from './Toolbar.module.scss';

const DEVICE_OPTIONS = [
    { value: 'none', label: 'None' },
    { value: 'serial_mouse', label: 'Serial mouse' },
    { value: 'loopback', label: 'Loopback' },
];

export function PeripheralsPanel(): React.ReactElement {
    const [comPorts, setComPorts] = useState<ComPortDevice[]>(state.comPorts.value);

    useSignalEffect(() => {
        setComPorts(state.comPorts.value);
    });

    return (
        <Stack gap="md" className={styles.panel}>
            <Text size="sm" fw={600}>
                Peripherals
            </Text>
            {Array.from({ length: COM_PORT_COUNT }, (_, i) => {
                const port = (i + 1) as 1 | 2 | 3 | 4;
                return (
                    <Select
                        key={port}
                        label={`COM${port}`}
                        data={DEVICE_OPTIONS}
                        value={comPorts[i] ?? 'none'}
                        onChange={(v) => {
                            if (v) {
                                state.setComPortDevice(port, v as ComPortDevice);
                            }
                        }}
                    />
                );
            })}
        </Stack>
    );
}

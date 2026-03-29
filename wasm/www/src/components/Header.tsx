import React, { useState } from 'react';
import { Notification, Text } from '@mantine/core';
import { useSignalEffect } from '@preact/signals-react';
import { state } from '../state';
import styles from '../App.module.scss';

export function Header(): React.ReactElement {
    const [error, setError] = useState<string | null>(state.error.value);
    const [mhz, setMhz] = useState(0);
    const [targetMhz, setTargetMhz] = useState(state.config.value.clock_hz / 1_000_000);

    useSignalEffect(() => {
        setError(state.error.value);
    });

    useSignalEffect(() => {
        if (!state.computer.value) {
            setMhz(0);
            return;
        }
        const id = setInterval(() => {
            state.sampleMhz();
        }, 500);
        return () => {
            clearInterval(id);
        };
    });

    useSignalEffect(() => {
        setMhz(state.perf.value);
    });

    useSignalEffect(() => {
        setTargetMhz(state.config.value.clock_hz / 1_000_000);
    });

    return (
        <div className={styles.header}>
            <img src="/logo.png" alt="Oxide86" className={styles.logo} />
            <div>
                <h1 className={styles.title}>Oxide86</h1>
                <p className={styles.subtitle}>x86 Rust Emulator</p>
            </div>
            {error ? (
                <Notification
                    className={styles.notification}
                    color="red"
                    withBorder
                    onClose={() => {
                        state.dismissError();
                    }}
                >
                    {error}
                </Notification>
            ) : mhz > 0 ? (
                <Text className={styles.perf} size="sm" c="dimmed">
                    {mhz.toFixed(2)} / {targetMhz.toFixed(2)} MHz
                </Text>
            ) : null}
        </div>
    );
}

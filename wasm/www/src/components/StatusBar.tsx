import React, { useState } from 'react';
import { useSignalEffect } from '@preact/signals-react';
import { state } from '../state';
import styles from './StatusBar.module.scss';

export function StatusBar(): React.ReactElement {
    const [message, setMessage] = useState(state.status.value.message);
    const [error, setError] = useState<string | null>(state.status.value.error);
    const [mhz, setMhz] = useState(0);

    useSignalEffect(() => {
        setMessage(state.status.value.message);
        setError(state.status.value.error);
    });

    // Start/stop the MHz sampling interval with the computer.
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

    return (
        <div className={styles.bar}>
            <span className={styles.message}>{message}</span>
            {error && (
                <span
                    className={styles.error}
                    onClick={() => {
                        state.dismissError();
                    }}
                    title="Click to dismiss"
                >
                    {error}
                </span>
            )}
            <span className={styles.perf}>{mhz > 0 ? `${mhz.toFixed(2)} MHz` : ''}</span>
        </div>
    );
}

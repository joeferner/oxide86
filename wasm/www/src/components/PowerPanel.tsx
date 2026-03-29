import React from 'react';
import { Stack, Button } from '@mantine/core';
import { state } from '../state';
import styles from './Toolbar.module.scss';

export function PowerPanel(): React.ReactElement {
    const running = state.computer.value !== null;

    return (
        <Stack gap="xs" className={styles.panel}>
            <Button
                size="xs"
                variant="default"
                color="green"
                disabled={running}
                onClick={() => {
                    void state.powerOn();
                }}
            >
                Power On
            </Button>
            <Button
                size="xs"
                variant="default"
                disabled={!running}
                onClick={() => {
                    state.reboot();
                }}
            >
                Reboot
            </Button>
        </Stack>
    );
}

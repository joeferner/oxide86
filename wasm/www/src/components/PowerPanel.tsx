import React from 'react';
import { Stack, Text, Button } from '@mantine/core';
import { state } from '../state';
import styles from './Toolbar.module.scss';

export interface PowerPanelProps {
    mode: 'power' | 'reboot';
    onClose: () => void;
}

export function PowerPanel({ mode, onClose }: PowerPanelProps): React.ReactElement {
    if (mode === 'power') {
        return (
            <Stack gap="xs" className={styles.panel}>
                <Text size="sm">Shut down the computer?</Text>
                <Button
                    size="xs"
                    color="red"
                    onClick={() => {
                        state.powerOff();
                        onClose();
                    }}
                >
                    Power Off
                </Button>
                <Button size="xs" variant="default" onClick={onClose}>
                    Cancel
                </Button>
            </Stack>
        );
    }

    return (
        <Stack gap="xs" className={styles.panel}>
            <Text size="sm">Reboot the computer?</Text>
            <Button
                size="xs"
                onClick={() => {
                    state.reboot();
                    onClose();
                }}
            >
                Reboot
            </Button>
            <Button size="xs" variant="default" onClick={onClose}>
                Cancel
            </Button>
        </Stack>
    );
}

import React, { useRef, useState } from 'react';
import { Stack, Text, Button } from '@mantine/core';
import { useSignalEffect } from '@preact/signals-react';
import { state } from '../state';
import styles from './Toolbar.module.scss';

export interface DrivePanelProps {
    label: string;
    drive: 0 | 1 | 'hdd';
    canEject: boolean;
}

export function DrivePanel({ label, drive, canEject }: DrivePanelProps): React.ReactElement {
    const fileInputRef = useRef<HTMLInputElement>(null);
    const [currentFile, setCurrentFile] = useState<File | null>(() =>
        drive === 0 ? state.floppyA.peek() : drive === 1 ? state.floppyB.peek() : state.hdd.peek()
    );

    useSignalEffect(() => {
        setCurrentFile(drive === 0 ? state.floppyA.value : drive === 1 ? state.floppyB.value : state.hdd.value);
    });

    const onFileChange = (e: React.ChangeEvent<HTMLInputElement>): void => {
        const file = e.target.files?.[0] ?? null;
        if (!file) {
            return;
        }
        if (drive === 'hdd') {
            state.setHdd(file);
        } else {
            void state.insertFloppy(drive, file);
        }
        e.target.value = '';
    };

    const onEject = (): void => {
        if (drive !== 'hdd') {
            state.ejectFloppy(drive);
        }
    };

    return (
        <Stack gap="xs" className={styles.panel}>
            <Text size="sm" fw={600}>
                {label}
            </Text>
            <Text size="xs" c="dimmed">
                {currentFile ? currentFile.name : 'Empty'}
            </Text>
            <input
                ref={fileInputRef}
                type="file"
                accept=".img,.ima,.bin"
                style={{ display: 'none' }}
                onChange={onFileChange}
            />
            <Button size="xs" variant="default" onClick={() => fileInputRef.current?.click()}>
                Load image…
            </Button>
            {canEject && (
                <Button size="xs" variant="outline" color="red" disabled={!currentFile} onClick={onEject}>
                    Eject
                </Button>
            )}
        </Stack>
    );
}

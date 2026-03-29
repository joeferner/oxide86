import React, { useRef, useState } from 'react';
import { ActionIcon, Popover, Stack, Button, Text, Tooltip } from '@mantine/core';
import { useSignal, useSignalEffect } from '@preact/signals-react';
import { state } from '../state';

export interface DriveButtonProps {
    label: string; // "A:", "B:", "C:"
    drive: 0 | 1 | 'hdd';
    icon: string; // bootstrap icon class
    canEject: boolean;
}

export function DriveButton({ label, drive, icon, canEject }: DriveButtonProps): React.ReactElement {
    const opened = useSignal(false);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const [currentFile, setCurrentFile] = useState<File | null>(null);

    useSignalEffect(() => {
        setCurrentFile(
            drive === 0 ? state.floppyA.value :
            drive === 1 ? state.floppyB.value :
            state.hdd.value
        );
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
        opened.value = false;
    };

    const onEject = (): void => {
        if (drive !== 'hdd') {
            state.ejectFloppy(drive);
        }
        opened.value = false;
    };

    return (
        <Popover
            opened={opened.value}
            onChange={(o) => {
                opened.value = o;
            }}
            position="right"
            withArrow
        >
            <Popover.Target>
                <Tooltip label={currentFile ? `${label} ${currentFile.name}` : label} position="right">
                    <ActionIcon
                        variant={currentFile ? 'filled' : 'subtle'}
                        size="lg"
                        onClick={() => {
                            opened.value = !opened.value;
                        }}
                        aria-label={label}
                    >
                        <i className={`bi ${currentFile ? `${icon}-fill` : icon}`} />
                    </ActionIcon>
                </Tooltip>
            </Popover.Target>

            <Popover.Dropdown>
                <Stack gap="xs">
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
                        <Button size="xs" variant="subtle" color="red" disabled={!currentFile} onClick={onEject}>
                            Eject
                        </Button>
                    )}
                </Stack>
            </Popover.Dropdown>
        </Popover>
    );
}

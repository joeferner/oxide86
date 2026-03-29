import React, { useState } from 'react';
import { ActionIcon, Tooltip } from '@mantine/core';
import { useSignalEffect } from '@preact/signals-react';
import { state } from '../state';

export interface DriveButtonProps {
    label: string; // "A:", "B:", "C:"
    drive: 0 | 1 | 'hdd';
    icon: string; // bootstrap icon class
    selected: boolean;
    onSelect: () => void;
}

export function DriveButton({ label, drive, icon, selected, onSelect }: DriveButtonProps): React.ReactElement {
    const [currentFile, setCurrentFile] = useState<File | null>(null);

    useSignalEffect(() => {
        setCurrentFile(drive === 0 ? state.floppyA.value : drive === 1 ? state.floppyB.value : state.hdd.value);
    });

    return (
        <Tooltip label={currentFile ? `${label} ${currentFile.name}` : label} position="left">
            <ActionIcon
                variant={selected ? 'light' : currentFile ? 'filled' : 'subtle'}
                size="lg"
                onClick={onSelect}
                aria-label={label}
            >
                <i className={`bi ${currentFile ? `${icon}-fill` : icon}`} />
            </ActionIcon>
        </Tooltip>
    );
}

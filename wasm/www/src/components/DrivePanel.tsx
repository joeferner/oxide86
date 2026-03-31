import React, { useRef, useState, useEffect } from 'react';
import { Stack, Text, Button, Divider, Loader, Tooltip, Group, Switch } from '@mantine/core';
import { useSignalEffect } from '@preact/signals-react';
import { state } from '../state';
import styles from './Toolbar.module.scss';

interface ImageEntry {
    name: string;
    description: string;
    url: string;
}

interface ImagesJson {
    floppy: ImageEntry[];
    hdd: ImageEntry[];
    cdrom: ImageEntry[];
}

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
    const [presetImages, setPresetImages] = useState<ImageEntry[]>([]);
    const [loadingUrl, setLoadingUrl] = useState<string | null>(null);
    const [isBootDrive, setIsBootDrive] = useState(() => state.bootDrive.peek() === drive);
    const [isRunning, setIsRunning] = useState(() => state.computer.peek() !== null);

    useSignalEffect(() => {
        setCurrentFile(drive === 0 ? state.floppyA.value : drive === 1 ? state.floppyB.value : state.hdd.value);
    });

    useSignalEffect(() => {
        setIsBootDrive(state.bootDrive.value === drive);
    });

    useSignalEffect(() => {
        setIsRunning(state.computer.value !== null);
    });

    useEffect(() => {
        fetch('/images.json')
            .then((r) => r.json() as Promise<ImagesJson>)
            .then((data) => {
                setPresetImages(drive === 'hdd' ? data.hdd : data.floppy);
            })
            .catch(() => {
                /* silently ignore */
            });
    }, [drive]);

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

    const onLoadPreset = async (entry: ImageEntry): Promise<void> => {
        setLoadingUrl(entry.url);
        try {
            const response = await fetch(entry.url);
            if (!response.ok) {
                throw new Error(`Failed to fetch image: ${response.status} ${response.statusText}`);
            }
            const blob = await response.blob();
            const filename = entry.url.split('/').pop() ?? entry.name;
            const file = new File([blob], filename, { type: 'application/octet-stream' });
            if (drive === 'hdd') {
                state.setHdd(file);
            } else {
                await state.insertFloppy(drive, file);
            }
        } catch (e) {
            state.setStatus('warning', String(e));
        } finally {
            setLoadingUrl(null);
        }
    };

    const onSave = (): void => {
        const computer = state.computer.peek();
        if (!computer || !currentFile) {
            return;
        }
        const driveId = drive === 'hdd' ? 0x80 : drive;
        const imageData = computer.get_disk_image(driveId);
        if (!imageData) {
            return;
        }
        const blob = new Blob([imageData], { type: 'application/octet-stream' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = currentFile.name;
        a.click();
        URL.revokeObjectURL(url);
    };

    const onBootToggle = (e: React.ChangeEvent<HTMLInputElement>): void => {
        state.setBootDrive(e.currentTarget.checked ? drive : null);
    };

    return (
        <Stack gap="xs" className={styles.panel}>
            <Group justify="space-between" align="center" wrap="nowrap">
                <Text size="sm" fw={600}>
                    {label}
                </Text>
                <Switch size="xs" label="Boot" checked={isBootDrive} onChange={onBootToggle} disabled={isRunning} />
            </Group>
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
            <Button size="xs" variant="default" disabled={!currentFile || !isRunning} onClick={onSave}>
                Save image…
            </Button>
            {canEject && (
                <Button size="xs" variant="outline" color="red" disabled={!currentFile} onClick={onEject}>
                    Eject
                </Button>
            )}
            {presetImages.length > 0 && (
                <>
                    <Divider label="Pre-made images" labelPosition="center" />
                    <Stack gap={4}>
                        {presetImages.map((entry) => (
                            <Tooltip
                                key={entry.url}
                                label={entry.description}
                                position="right"
                                withArrow
                                multiline
                                w={200}
                            >
                                <Button
                                    size="xs"
                                    variant="subtle"
                                    justify="left"
                                    fullWidth
                                    disabled={loadingUrl !== null}
                                    rightSection={loadingUrl === entry.url ? <Loader size={12} /> : null}
                                    onClick={() => void onLoadPreset(entry)}
                                >
                                    {entry.name}
                                </Button>
                            </Tooltip>
                        ))}
                    </Stack>
                </>
            )}
        </Stack>
    );
}

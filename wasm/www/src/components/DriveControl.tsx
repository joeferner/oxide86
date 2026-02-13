import { useState, useEffect } from 'react';
import { Group, Button, FileButton, Text, ActionIcon, Tooltip } from '@mantine/core';
import { Emu86Computer } from '../../pkg/emu86_wasm';
import styles from './ControlGroup.module.scss';

interface DriveControlProps {
    computer: Emu86Computer | null;
    onStatusUpdate: (message: string) => void;
    onManageDrive: (driveNumber: number) => void;
}

async function loadFile(file: File): Promise<Uint8Array> {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = (e) => {
            resolve(new Uint8Array(e.target?.result as ArrayBuffer));
        };
        reader.onerror = reject;
        reader.readAsArrayBuffer(file);
    });
}

export function DriveControl({
    computer,
    onStatusUpdate,
    onManageDrive,
}: DriveControlProps): React.ReactElement | null {
    const [floppyAFile, setFloppyAFile] = useState<File | null>(null);
    const [floppyBFile, setFloppyBFile] = useState<File | null>(null);
    const [hddFile, setHddFile] = useState<File | null>(null);

    // Re-mount previously loaded drives whenever the computer instance is replaced
    useEffect(() => {
        if (!computer) {
            return;
        }
        const remount = async (): Promise<void> => {
            if (floppyAFile) {
                try {
                    computer.load_floppy(0, await loadFile(floppyAFile));
                } catch {
                    setFloppyAFile(null);
                }
            }
            if (floppyBFile) {
                try {
                    computer.load_floppy(1, await loadFile(floppyBFile));
                } catch {
                    setFloppyBFile(null);
                }
            }
            if (hddFile) {
                try {
                    computer.add_hard_drive(await loadFile(hddFile));
                } catch {
                    setHddFile(null);
                }
            }
        };
        void remount();
        // Only re-run when the computer instance itself changes, not when file state changes
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [computer]);

    const handleDownloadDrive = (driveType: 'floppy' | 'hdd', driveNumber: number): void => {
        if (!computer) {
            return;
        }

        try {
            const data =
                driveType === 'floppy'
                    ? computer.get_floppy_data(driveNumber)
                    : computer.get_hard_drive_data(driveNumber - 0x80);

            if (!data) {
                throw new Error('No data returned');
            }

            // Create a new Uint8Array to ensure proper ArrayBuffer type
            const arrayData = new Uint8Array(data);
            const blob = new Blob([arrayData], { type: 'application/octet-stream' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            const driveLetter =
                driveType === 'floppy'
                    ? String.fromCharCode(65 + driveNumber)
                    : String.fromCharCode(67 + (driveNumber - 0x80));
            a.download = `drive_${driveLetter}.img`;
            a.click();
            URL.revokeObjectURL(url);
            onStatusUpdate(`Downloaded drive ${driveLetter}:`);
        } catch (e) {
            onStatusUpdate(`Error downloading disk: ${e}`);
            console.error(e);
        }
    };

    const handleFloppyAChange = async (file: File | null): Promise<void> => {
        if (!file) {
            return;
        }

        setFloppyAFile(file);
        try {
            onStatusUpdate('Loading floppy A...');
            const data = await loadFile(file);
            computer?.load_floppy(0, data);
            onStatusUpdate(`Loaded floppy A: ${file.name} (${data.length} bytes)`);
        } catch (e) {
            onStatusUpdate(`Error loading floppy A: ${e}`);
            console.error(e);
            setFloppyAFile(null);
        }
    };

    const handleEjectFloppyA = (): void => {
        try {
            computer?.eject_floppy(0);
            setFloppyAFile(null);
            onStatusUpdate('Floppy A ejected');
        } catch (e) {
            onStatusUpdate(`Error ejecting floppy A: ${e}`);
            console.error(e);
        }
    };

    const handleFloppyBChange = async (file: File | null): Promise<void> => {
        if (!file) {
            return;
        }

        setFloppyBFile(file);
        try {
            onStatusUpdate('Loading floppy B...');
            const data = await loadFile(file);
            computer?.load_floppy(1, data);
            onStatusUpdate(`Loaded floppy B: ${file.name} (${data.length} bytes)`);
        } catch (e) {
            onStatusUpdate(`Error loading floppy B: ${e}`);
            console.error(e);
            setFloppyBFile(null);
        }
    };

    const handleEjectFloppyB = (): void => {
        try {
            computer?.eject_floppy(1);
            setFloppyBFile(null);
            onStatusUpdate('Floppy B ejected');
        } catch (e) {
            onStatusUpdate(`Error ejecting floppy B: ${e}`);
            console.error(e);
        }
    };

    const handleHDDChange = async (file: File | null): Promise<void> => {
        if (!file) {
            return;
        }

        setHddFile(file);
        try {
            onStatusUpdate('Loading hard drive C...');
            const data = await loadFile(file);
            computer?.add_hard_drive(data);
            onStatusUpdate(`Loaded hard drive C: ${file.name} (${data.length} bytes)`);
        } catch (e) {
            onStatusUpdate(`Error loading hard drive: ${e}`);
            console.error(e);
            setHddFile(null);
        }
    };

    if (!computer) {
        return null;
    }

    return (
        <>
            <div className={styles.controlGroup}>
                <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>
                    Floppy Drive A:
                </Text>
                <Group gap="xs">
                    <FileButton
                        key={floppyAFile?.name ?? 'empty-a'}
                        onChange={(v) => {
                            void handleFloppyAChange(v);
                        }}
                        accept=".img,.ima,.dsk"
                    >
                        {(props) => (
                            <Button {...props} size="compact-sm" variant="default">
                                {floppyAFile ? floppyAFile.name : 'Choose File'}
                            </Button>
                        )}
                    </FileButton>
                    <Tooltip label="Eject A:">
                        <ActionIcon onClick={handleEjectFloppyA} size="md" color="red">
                            <i className="bi bi-eject"></i>
                        </ActionIcon>
                    </Tooltip>
                    <Tooltip label="Manage Drive A:">
                        <ActionIcon
                            onClick={() => {
                                onManageDrive(0);
                            }}
                            size="md"
                            color="blue"
                        >
                            <i className="bi bi-gear-fill"></i>
                        </ActionIcon>
                    </Tooltip>
                    <Tooltip label="Download Drive A:">
                        <ActionIcon
                            onClick={() => {
                                handleDownloadDrive('floppy', 0);
                            }}
                            size="md"
                            color="blue"
                        >
                            <i className="bi bi-download"></i>
                        </ActionIcon>
                    </Tooltip>
                </Group>
            </div>

            <div className={styles.controlGroup}>
                <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>
                    Floppy Drive B:
                </Text>
                <Group gap="xs">
                    <FileButton
                        key={floppyBFile?.name ?? 'empty-b'}
                        onChange={(v) => {
                            void handleFloppyBChange(v);
                        }}
                        accept=".img,.ima,.dsk"
                    >
                        {(props) => (
                            <Button {...props} size="compact-sm" variant="default">
                                {floppyBFile ? floppyBFile.name : 'Choose File'}
                            </Button>
                        )}
                    </FileButton>
                    <Tooltip label="Eject B:">
                        <ActionIcon onClick={handleEjectFloppyB} size="md" color="red">
                            <i className="bi bi-eject"></i>
                        </ActionIcon>
                    </Tooltip>
                    <Tooltip label="Manage Drive B:">
                        <ActionIcon
                            onClick={() => {
                                onManageDrive(1);
                            }}
                            size="md"
                            color="blue"
                        >
                            <i className="bi bi-gear-fill"></i>
                        </ActionIcon>
                    </Tooltip>
                    <Tooltip label="Download Drive B:">
                        <ActionIcon
                            onClick={() => {
                                handleDownloadDrive('floppy', 1);
                            }}
                            size="md"
                            color="blue"
                        >
                            <i className="bi bi-download"></i>
                        </ActionIcon>
                    </Tooltip>
                </Group>
            </div>

            <div className={styles.controlGroup}>
                <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>
                    Hard Drive C:
                </Text>
                <Group gap="xs">
                    <FileButton
                        onChange={(v) => {
                            void handleHDDChange(v);
                        }}
                        accept=".img,.ima,.dsk,.vhd"
                    >
                        {(props) => (
                            <Button {...props} size="compact-sm" variant="default">
                                {hddFile ? hddFile.name : 'Choose File'}
                            </Button>
                        )}
                    </FileButton>
                    <Tooltip label="Manage Drive C:">
                        <ActionIcon
                            onClick={() => {
                                onManageDrive(0x80);
                            }}
                            size="md"
                            color="blue"
                        >
                            <i className="bi bi-gear-fill"></i>
                        </ActionIcon>
                    </Tooltip>
                    <Tooltip label="Download Drive C:">
                        <ActionIcon
                            onClick={() => {
                                handleDownloadDrive('hdd', 0x80);
                            }}
                            size="md"
                            color="blue"
                        >
                            <i className="bi bi-download"></i>
                        </ActionIcon>
                    </Tooltip>
                </Group>
            </div>
        </>
    );
}

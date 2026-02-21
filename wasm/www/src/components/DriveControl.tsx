import { useSignal, useSignalEffect } from '@preact/signals-react';
import { Group, Button, FileButton, Text, ActionIcon, Tooltip } from '@mantine/core';
import { computer, status } from '../emulatorState';
import styles from './ControlGroup.module.scss';

interface DriveControlProps {
    onManageDrive: (driveNumber: number) => void;
    floppyALabel?: string | null;
    floppyBLabel?: string | null;
    onFloppyEjected?: (slot: number) => void;
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
    onManageDrive,
    floppyALabel,
    floppyBLabel,
    onFloppyEjected,
}: DriveControlProps): React.ReactElement | null {
    const floppyAFile = useSignal<File | null>(null);
    const floppyBFile = useSignal<File | null>(null);
    const hddFile = useSignal<File | null>(null);
    const cdromFile = useSignal<File | null>(null);

    // Re-mount previously loaded drives whenever the computer instance is replaced
    useSignalEffect(() => {
        const comp = computer.value;
        if (!comp) {
            return;
        }

        const remount = async (): Promise<void> => {
            // Use .peek() to read file signals without subscribing to them.
            // This effect should only rerun when `computer.value` changes (e.g. on reset),
            // not when the user loads a new drive (which is handled by the handlers directly).

            const floppyAFilePeek = floppyAFile.peek();
            if (floppyAFilePeek) {
                try {
                    comp.load_floppy(0, await loadFile(floppyAFilePeek));
                } catch {
                    floppyAFile.value = null;
                }
            }

            const floppyBFilePeek = floppyBFile.peek();
            if (floppyBFilePeek) {
                try {
                    comp.load_floppy(1, await loadFile(floppyBFilePeek));
                } catch {
                    floppyBFile.value = null;
                }
            }

            const hddFilePeek = hddFile.peek();
            if (hddFilePeek) {
                try {
                    comp.add_hard_drive(await loadFile(hddFilePeek));
                } catch {
                    hddFile.value = null;
                }
            }

            const cdromFilePeek = cdromFile.peek();
            if (cdromFilePeek) {
                try {
                    comp.load_cdrom(0, await loadFile(cdromFilePeek));
                } catch {
                    cdromFile.value = null;
                }
            }
        };
        void remount();
    });

    const handleDownloadDrive = (driveType: 'floppy' | 'hdd', driveNumber: number): void => {
        const comp = computer.value;
        if (!comp) {
            return;
        }

        try {
            const data =
                driveType === 'floppy'
                    ? comp.get_floppy_data(driveNumber)
                    : comp.get_hard_drive_data(driveNumber - 0x80);

            if (!data) {
                throw new Error('No data returned');
            }

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
            status.value = `Downloaded drive ${driveLetter}:`;
        } catch (e) {
            status.value = `Error downloading disk: ${e}`;
            console.error(e);
        }
    };

    const handleFloppyAChange = async (file: File | null): Promise<void> => {
        if (!file) {
            return;
        }
        floppyAFile.value = file;
        try {
            status.value = 'Loading floppy A...';
            const data = await loadFile(file);
            computer.value?.load_floppy(0, data);
            status.value = `Loaded floppy A: ${file.name} (${data.length} bytes)`;
        } catch (e) {
            status.value = `Error loading floppy A: ${e}`;
            console.error(e);
            floppyAFile.value = null;
        }
    };

    const handleEjectFloppyA = (): void => {
        try {
            computer.value?.eject_floppy(0);
            floppyAFile.value = null;
            onFloppyEjected?.(0);
            status.value = 'Floppy A ejected';
        } catch (e) {
            status.value = `Error ejecting floppy A: ${e}`;
            console.error(e);
        }
    };

    const handleFloppyBChange = async (file: File | null): Promise<void> => {
        if (!file) {
            return;
        }
        floppyBFile.value = file;
        try {
            status.value = 'Loading floppy B...';
            const data = await loadFile(file);
            computer.value?.load_floppy(1, data);
            status.value = `Loaded floppy B: ${file.name} (${data.length} bytes)`;
        } catch (e) {
            status.value = `Error loading floppy B: ${e}`;
            console.error(e);
            floppyBFile.value = null;
        }
    };

    const handleEjectFloppyB = (): void => {
        try {
            computer.value?.eject_floppy(1);
            floppyBFile.value = null;
            onFloppyEjected?.(1);
            status.value = 'Floppy B ejected';
        } catch (e) {
            status.value = `Error ejecting floppy B: ${e}`;
            console.error(e);
        }
    };

    const handleHDDChange = async (file: File | null): Promise<void> => {
        if (!file) {
            return;
        }
        hddFile.value = file;
        try {
            status.value = 'Loading hard drive C...';
            const data = await loadFile(file);
            computer.value?.add_hard_drive(data);
            status.value = `Loaded hard drive C: ${file.name} (${data.length} bytes)`;
        } catch (e) {
            status.value = `Error loading hard drive: ${e}`;
            console.error(e);
            hddFile.value = null;
        }
    };

    const handleCdRomChange = async (file: File | null): Promise<void> => {
        if (!file) {
            return;
        }
        cdromFile.value = file;
        try {
            status.value = 'Loading CD-ROM...';
            const data = await loadFile(file);
            computer.value?.load_cdrom(0, data);
            status.value = `Loaded CD-ROM: ${file.name} (${data.length} bytes)`;
        } catch (e) {
            status.value = `Error loading CD-ROM: ${e}`;
            console.error(e);
            cdromFile.value = null;
        }
    };

    const handleEjectCdRom = (): void => {
        try {
            computer.value?.eject_cdrom_slot(0);
            cdromFile.value = null;
            status.value = 'CD-ROM ejected';
        } catch (e) {
            status.value = `Error ejecting CD-ROM: ${e}`;
            console.error(e);
        }
    };

    if (!computer.value) {
        return null;
    }

    return (
        <>
            <div className={styles.controlGroup}>
                <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>
                    Floppy Drive A:
                </Text>
                <Group gap="xs">
                    {floppyAFile.value === null && floppyALabel ? (
                        <Button size="compact-sm" variant="filled" color="teal" disabled>
                            {floppyALabel}
                        </Button>
                    ) : (
                        <FileButton
                            key={floppyAFile.value?.name ?? 'empty-a'}
                            onChange={(v) => {
                                void handleFloppyAChange(v);
                            }}
                            accept=".img,.ima,.dsk"
                        >
                            {(props) => (
                                <Button {...props} size="compact-sm" variant="default">
                                    {floppyAFile.value ? floppyAFile.value.name : 'Choose File'}
                                </Button>
                            )}
                        </FileButton>
                    )}
                    <Tooltip label="Eject A:">
                        <ActionIcon
                            onClick={handleEjectFloppyA}
                            size="md"
                            color="red"
                            disabled={floppyAFile.value === null && !floppyALabel}
                        >
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
                    {floppyBFile.value === null && floppyBLabel ? (
                        <Button size="compact-sm" variant="filled" color="teal" disabled>
                            {floppyBLabel}
                        </Button>
                    ) : (
                        <FileButton
                            key={floppyBFile.value?.name ?? 'empty-b'}
                            onChange={(v) => {
                                void handleFloppyBChange(v);
                            }}
                            accept=".img,.ima,.dsk"
                        >
                            {(props) => (
                                <Button {...props} size="compact-sm" variant="default">
                                    {floppyBFile.value ? floppyBFile.value.name : 'Choose File'}
                                </Button>
                            )}
                        </FileButton>
                    )}
                    <Tooltip label="Eject B:">
                        <ActionIcon
                            onClick={handleEjectFloppyB}
                            size="md"
                            color="red"
                            disabled={floppyBFile.value === null && !floppyBLabel}
                        >
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
                                {hddFile.value ? hddFile.value.name : 'Choose File'}
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

            <div className={styles.controlGroup}>
                <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>
                    CD-ROM Drive:
                </Text>
                <Group gap="xs">
                    <FileButton
                        key={cdromFile.value?.name ?? 'empty-cdrom'}
                        onChange={(v) => {
                            void handleCdRomChange(v);
                        }}
                        accept=".iso"
                    >
                        {(props) => (
                            <Button {...props} size="compact-sm" variant="default">
                                {cdromFile.value ? cdromFile.value.name : 'Choose ISO'}
                            </Button>
                        )}
                    </FileButton>
                    <Tooltip label="Eject CD-ROM">
                        <ActionIcon
                            onClick={handleEjectCdRom}
                            size="md"
                            color="red"
                            disabled={cdromFile.value === null}
                        >
                            <i className="bi bi-eject"></i>
                        </ActionIcon>
                    </Tooltip>
                </Group>
            </div>
        </>
    );
}

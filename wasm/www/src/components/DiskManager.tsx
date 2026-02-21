import { useSignal } from '@preact/signals-react';
import {
    Modal,
    Table,
    Button,
    Group,
    Stack,
    Breadcrumbs,
    Anchor,
    FileButton,
    ActionIcon,
    Tooltip,
    Text,
    Select,
    TextInput,
} from '@mantine/core';
import { create_floppy_image, create_hdd_image } from '../../pkg/oxide86_wasm';
import { computer, status } from '../emulatorState';

interface FileEntry {
    name: string;
    size: number;
    isDirectory: boolean;
    date: string;
    time: string;
    attributes: number;
}

interface DiskManagerProps {
    opened: boolean;
    onClose: () => void;
    driveNumber: number;
    onFloppyCreated?: (slot: number, label: string) => void;
}

interface DeleteConfirmation {
    drive: number;
    path: string;
    name: string;
}

export function DiskManager({ opened, onClose, driveNumber, onFloppyCreated }: DiskManagerProps): React.ReactElement {
    const currentPath = useSignal<string>('/');
    const files = useSignal<FileEntry[]>([]);
    const loading = useSignal(false);
    const deleteConfirmation = useSignal<DeleteConfirmation | null>(null);
    const createDiskOpened = useSignal(false);
    const selectedSize = useSignal<string | null>(null);
    const diskLabel = useSignal('');

    const isFloppy = driveNumber < 0x80;

    const floppySizeOptions = [
        { value: '1440', label: '1.44 MB (3.5" HD)' },
        { value: '720', label: '720 KB (3.5" DD)' },
        { value: '360', label: '360 KB (5.25" DD)' },
        { value: '160', label: '160 KB (5.25" SS)' },
    ];

    const hddSizeOptions = [
        { value: '10', label: '10 MB' },
        { value: '20', label: '20 MB' },
        { value: '32', label: '32 MB' },
        { value: '64', label: '64 MB' },
        { value: '128', label: '128 MB' },
    ];

    const sizeOptions = isFloppy ? floppySizeOptions : hddSizeOptions;

    const getDriveLetter = (drive: number): string => {
        if (drive === 0) {
            return 'A';
        }
        if (drive === 1) {
            return 'B';
        }
        if (drive >= 0x80) {
            return String.fromCharCode(67 + (drive - 0x80));
        }
        return '?';
    };

    const browseDisk = (drive: number, path: string): void => {
        const comp = computer.value;
        if (!comp) {
            return;
        }

        loading.value = true;
        try {
            const fileList = comp.list_directory(drive, path) as unknown as FileEntry[];

            fileList.sort((a, b) => {
                if (a.isDirectory && !b.isDirectory) {
                    return -1;
                }
                if (!a.isDirectory && b.isDirectory) {
                    return 1;
                }
                return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
            });

            files.value = fileList;
        } catch (e) {
            status.value = `Error browsing ${getDriveLetter(drive)}: ${e}`;
            files.value = [];
        } finally {
            loading.value = false;
        }
    };

    // Refresh when dialog opens or drive/path changes
    if (opened && computer.value) {
        browseDisk(driveNumber, currentPath.value);
    }

    const downloadFile = (drive: number, filePath: string, fileName: string): void => {
        const comp = computer.value;
        if (!comp) {
            return;
        }

        loading.value = true;
        try {
            const data = comp.read_file_from_disk(drive, filePath);
            const arrayData = new Uint8Array(data);
            const blob = new Blob([arrayData], { type: 'application/octet-stream' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = fileName;
            a.click();
            URL.revokeObjectURL(url);
            status.value = `Downloaded ${fileName}`;
        } catch (e) {
            status.value = `Error downloading file: ${e}`;
        } finally {
            loading.value = false;
        }
    };

    const uploadFiles = async (selectedFiles: File[]): Promise<void> => {
        const comp = computer.value;
        if (!comp || selectedFiles.length === 0) {
            return;
        }

        loading.value = true;
        let uploaded = 0;
        const errors: string[] = [];

        for (const file of selectedFiles) {
            try {
                const arrayBuffer = await file.arrayBuffer();
                const data = new Uint8Array(arrayBuffer);
                const targetPath = currentPath.value === '/' ? `/${file.name}` : `${currentPath.value}/${file.name}`;
                comp.write_file_to_disk(driveNumber, targetPath, data);
                uploaded++;
            } catch (e) {
                errors.push(`${file.name}: ${e}`);
            }
        }

        if (errors.length > 0) {
            status.value = `Uploaded ${uploaded}/${selectedFiles.length} files. Errors: ${errors.join(', ')}`;
        } else {
            status.value = `Uploaded ${uploaded} file${uploaded !== 1 ? 's' : ''} to ${getDriveLetter(driveNumber)}:${currentPath.value}`;
        }
        browseDisk(driveNumber, currentPath.value);
        loading.value = false;
    };

    const deleteItem = (drive: number, path: string, name: string): void => {
        deleteConfirmation.value = { drive, path, name };
    };

    const confirmDelete = (): void => {
        const comp = computer.value;
        const conf = deleteConfirmation.value;
        if (!comp || !conf) {
            return;
        }

        const { drive, path, name } = conf;
        deleteConfirmation.value = null;
        loading.value = true;

        try {
            comp.delete_from_disk(drive, path);
            status.value = `Deleted ${name}`;
            browseDisk(driveNumber, currentPath.value);
        } catch (e) {
            status.value = `Error deleting: ${e}`;
        } finally {
            loading.value = false;
        }
    };

    const navigateToDirectory = (dirName: string): void => {
        if (dirName === '..') {
            const parts = currentPath.value.split('/').filter((p) => p);
            parts.pop();
            currentPath.value = parts.length === 0 ? '/' : '/' + parts.join('/');
        } else {
            currentPath.value = currentPath.value === '/' ? `/${dirName}` : `${currentPath.value}/${dirName}`;
        }
    };

    const createNewDisk = (): void => {
        const comp = computer.value;
        if (!selectedSize.value || !comp) {
            return;
        }

        loading.value = true;
        try {
            const label = diskLabel.value.trim() || undefined;
            const size = parseInt(selectedSize.value);

            if (isFloppy) {
                const data = new Uint8Array(create_floppy_image(size, label));
                comp.load_floppy(driveNumber, data);
                status.value = `Created ${selectedSize.value}KB floppy on ${getDriveLetter(driveNumber)}:`;
                onFloppyCreated?.(driveNumber, label ?? `New Disk (${selectedSize.value}KB)`);
                currentPath.value = '/';
                browseDisk(driveNumber, '/');
            } else {
                const data = new Uint8Array(create_hdd_image(size, label));
                const blob = new Blob([data], { type: 'application/octet-stream' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = `drive_${getDriveLetter(driveNumber)}_new.img`;
                a.click();
                URL.revokeObjectURL(url);
                status.value = `Created and downloaded ${selectedSize.value}MB HDD image`;
            }

            createDiskOpened.value = false;
            selectedSize.value = null;
            diskLabel.value = '';
        } catch (e) {
            status.value = `Error creating disk: ${e}`;
        } finally {
            loading.value = false;
        }
    };

    const formatSize = (bytes: number): string => {
        if (bytes < 1024) {
            return `${bytes} B`;
        }
        if (bytes < 1024 * 1024) {
            return `${(bytes / 1024).toFixed(1)} KB`;
        }
        return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    };

    const pathParts = currentPath.value.split('/').filter((p) => p);
    const breadcrumbs = [
        <Anchor
            key="root"
            onClick={() => {
                currentPath.value = '/';
            }}
            size="sm"
        >
            {getDriveLetter(driveNumber)}:
        </Anchor>,
        ...pathParts.map((part, idx) => (
            <Anchor
                key={`path-${pathParts.slice(0, idx + 1).join('/')}`}
                onClick={() => {
                    currentPath.value = '/' + pathParts.slice(0, idx + 1).join('/');
                }}
                size="sm"
            >
                {part}
            </Anchor>
        )),
    ];

    return (
        <>
            <Modal
                opened={opened}
                onClose={onClose}
                size="xl"
                title={`Disk Manager - Drive ${getDriveLetter(driveNumber)}:`}
            >
                <Stack gap="md">
                    <Group justify="flex-end">
                        <FileButton
                            onChange={(uploadedFiles) => {
                                void uploadFiles(uploadedFiles);
                            }}
                            accept="*/*"
                            multiple
                        >
                            {(props) => (
                                <Button {...props} size="sm" disabled={loading.value} color="blue">
                                    Upload File
                                </Button>
                            )}
                        </FileButton>
                        <Button
                            onClick={() => {
                                createDiskOpened.value = true;
                            }}
                            size="sm"
                            disabled={loading.value}
                            color="green"
                        >
                            Create New Disk
                        </Button>
                        <Button
                            onClick={() => {
                                browseDisk(driveNumber, currentPath.value);
                            }}
                            size="sm"
                            disabled={loading.value}
                            color="blue"
                        >
                            Refresh
                        </Button>
                    </Group>

                    <Breadcrumbs>{breadcrumbs}</Breadcrumbs>

                    <Table>
                        <Table.Thead>
                            <Table.Tr>
                                <Table.Th>Name</Table.Th>
                                <Table.Th>Size</Table.Th>
                                <Table.Th>Date</Table.Th>
                                <Table.Th>Actions</Table.Th>
                            </Table.Tr>
                        </Table.Thead>
                        <Table.Tbody>
                            {currentPath.value !== '/' && (
                                <Table.Tr>
                                    <Table.Td>
                                        <Anchor
                                            onClick={() => {
                                                navigateToDirectory('..');
                                            }}
                                        >
                                            📁 ..
                                        </Anchor>
                                    </Table.Td>
                                    <Table.Td>-</Table.Td>
                                    <Table.Td>-</Table.Td>
                                    <Table.Td>-</Table.Td>
                                </Table.Tr>
                            )}
                            {files.value.map((file) => {
                                const fullPath =
                                    currentPath.value === '/' ? `/${file.name}` : `${currentPath.value}/${file.name}`;
                                return (
                                    <Table.Tr key={file.name}>
                                        <Table.Td>
                                            {file.isDirectory ? (
                                                <Anchor
                                                    onClick={() => {
                                                        navigateToDirectory(file.name);
                                                    }}
                                                >
                                                    📁 {file.name}
                                                </Anchor>
                                            ) : (
                                                <span>📄 {file.name}</span>
                                            )}
                                        </Table.Td>
                                        <Table.Td>{file.isDirectory ? '-' : formatSize(file.size)}</Table.Td>
                                        <Table.Td>{file.date}</Table.Td>
                                        <Table.Td>
                                            <Group gap="xs">
                                                {!file.isDirectory && (
                                                    <Tooltip label="Download">
                                                        <ActionIcon
                                                            size="sm"
                                                            color="blue"
                                                            variant="light"
                                                            onClick={() => {
                                                                downloadFile(driveNumber, fullPath, file.name);
                                                            }}
                                                            disabled={loading.value}
                                                        >
                                                            <i className="bi bi-download"></i>
                                                        </ActionIcon>
                                                    </Tooltip>
                                                )}
                                                <Tooltip label="Delete">
                                                    <ActionIcon
                                                        size="sm"
                                                        color="red"
                                                        variant="light"
                                                        onClick={() => {
                                                            deleteItem(driveNumber, fullPath, file.name);
                                                        }}
                                                        disabled={loading.value}
                                                    >
                                                        <i className="bi bi-trash"></i>
                                                    </ActionIcon>
                                                </Tooltip>
                                            </Group>
                                        </Table.Td>
                                    </Table.Tr>
                                );
                            })}
                        </Table.Tbody>
                    </Table>

                    {files.value.length === 0 && !loading.value && (
                        <div style={{ textAlign: 'center', padding: '2rem', color: '#666' }}>No files found</div>
                    )}
                </Stack>
            </Modal>

            <Modal
                opened={deleteConfirmation.value !== null}
                onClose={() => {
                    deleteConfirmation.value = null;
                }}
                title="Confirm Deletion"
                size="sm"
            >
                <Stack gap="md">
                    <Text>
                        Are you sure you want to delete <strong>{deleteConfirmation.value?.name}</strong>?
                    </Text>
                    <Group justify="flex-end">
                        <Button
                            variant="default"
                            onClick={() => {
                                deleteConfirmation.value = null;
                            }}
                        >
                            Cancel
                        </Button>
                        <Button color="red" onClick={confirmDelete}>
                            Delete
                        </Button>
                    </Group>
                </Stack>
            </Modal>

            <Modal
                opened={createDiskOpened.value}
                onClose={() => {
                    createDiskOpened.value = false;
                    selectedSize.value = null;
                    diskLabel.value = '';
                }}
                title={isFloppy ? 'Create New Floppy Disk' : 'Create New Hard Drive Image'}
                size="sm"
            >
                <Stack gap="md">
                    <Select
                        label="Disk Size"
                        placeholder="Select size"
                        data={sizeOptions}
                        value={selectedSize.value}
                        onChange={(v) => {
                            selectedSize.value = v;
                        }}
                    />
                    <TextInput
                        label="Volume Label (optional, max 11 chars)"
                        maxLength={11}
                        value={diskLabel.value}
                        onChange={(e) => {
                            diskLabel.value = e.currentTarget.value.toUpperCase();
                        }}
                    />
                    {!isFloppy && (
                        <Text size="sm" c="dimmed">
                            The HDD image will be downloaded. Load it via Drive Control to use it.
                        </Text>
                    )}
                    <Group justify="flex-end">
                        <Button
                            variant="default"
                            onClick={() => {
                                createDiskOpened.value = false;
                                selectedSize.value = null;
                                diskLabel.value = '';
                            }}
                        >
                            Cancel
                        </Button>
                        <Button color="green" disabled={!selectedSize.value || loading.value} onClick={createNewDisk}>
                            {isFloppy ? 'Create & Load' : 'Create & Download'}
                        </Button>
                    </Group>
                </Stack>
            </Modal>
        </>
    );
}

import { useState, useEffect, useCallback } from 'react';
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
} from '@mantine/core';
import { Emu86Computer } from '../../pkg/emu86_wasm';

interface FileEntry {
    name: string;
    size: number;
    isDirectory: boolean;
    date: string;
    time: string;
    attributes: number;
}

interface DiskManagerProps {
    computer: Emu86Computer | null;
    opened: boolean;
    onClose: () => void;
    onStatusUpdate: (message: string) => void;
    driveNumber: number;
}

interface DeleteConfirmation {
    drive: number;
    path: string;
    name: string;
}

export function DiskManager({
    computer,
    opened,
    onClose,
    onStatusUpdate,
    driveNumber,
}: DiskManagerProps): React.ReactElement {
    const [currentPath, setCurrentPath] = useState<string>('/');
    const [files, setFiles] = useState<FileEntry[]>([]);
    const [loading, setLoading] = useState(false);
    const [deleteConfirmation, setDeleteConfirmation] = useState<DeleteConfirmation | null>(null);

    // Get drive letter from drive number
    const getDriveLetter = useCallback((drive: number): string => {
        if (drive === 0) {
            return 'A';
        }
        if (drive === 1) {
            return 'B';
        }
        if (drive >= 0x80) {
            return String.fromCharCode(67 + (drive - 0x80));
        } // C, D, E, etc.
        return '?';
    }, []);

    // Browse disk directory
    const browseDisk = useCallback(
        (drive: number, path: string): void => {
            if (!computer) {
                return;
            }

            setLoading(true);
            try {
                const fileList = computer.list_directory(drive, path) as unknown as FileEntry[];

                // Sort files: directories first, then files, both alphabetically (case-insensitive)
                fileList.sort((a, b) => {
                    // Directories come before files
                    if (a.isDirectory && !b.isDirectory) {
                        return -1;
                    }
                    if (!a.isDirectory && b.isDirectory) {
                        return 1;
                    }

                    // Sort by name (case-insensitive)
                    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
                });

                setFiles(fileList);
            } catch (e) {
                onStatusUpdate(`Error browsing ${getDriveLetter(drive)}: ${e}`);
                setFiles([]);
            } finally {
                setLoading(false);
            }
        },
        [computer, onStatusUpdate, getDriveLetter]
    );

    // Refresh directory listing when drive or path changes
    useEffect(() => {
        if (opened && computer) {
            browseDisk(driveNumber, currentPath);
        }
    }, [opened, computer, driveNumber, currentPath, browseDisk]);

    // Download individual file
    const downloadFile = (drive: number, filePath: string, fileName: string): void => {
        if (!computer) {
            return;
        }

        setLoading(true);
        try {
            const data = computer.read_file_from_disk(drive, filePath);
            // Create a new Uint8Array to ensure proper ArrayBuffer type
            const arrayData = new Uint8Array(data);
            const blob = new Blob([arrayData], { type: 'application/octet-stream' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = fileName;
            a.click();
            URL.revokeObjectURL(url);
            onStatusUpdate(`Downloaded ${fileName}`);
        } catch (e) {
            onStatusUpdate(`Error downloading file: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    // Upload file to disk
    const uploadFile = async (file: File | null): Promise<void> => {
        if (!computer || !file) {
            return;
        }

        setLoading(true);
        try {
            const arrayBuffer = await file.arrayBuffer();
            const data = new Uint8Array(arrayBuffer);
            const targetPath = currentPath === '/' ? `/${file.name}` : `${currentPath}/${file.name}`;
            computer.write_file_to_disk(driveNumber, targetPath, data);
            onStatusUpdate(`Uploaded ${file.name} to ${getDriveLetter(driveNumber)}:${targetPath}`);
            browseDisk(driveNumber, currentPath); // Refresh listing
        } catch (e) {
            onStatusUpdate(`Error uploading file: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    // Show delete confirmation dialog
    const deleteItem = (drive: number, path: string, name: string): void => {
        setDeleteConfirmation({ drive, path, name });
    };

    // Perform actual deletion after confirmation
    const confirmDelete = (): void => {
        if (!computer || !deleteConfirmation) {
            return;
        }

        const { drive, path, name } = deleteConfirmation;
        setDeleteConfirmation(null);
        setLoading(true);

        try {
            computer.delete_from_disk(drive, path);
            onStatusUpdate(`Deleted ${name}`);
            browseDisk(driveNumber, currentPath); // Refresh listing
        } catch (e) {
            onStatusUpdate(`Error deleting: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    // Navigate to directory
    const navigateToDirectory = (dirName: string): void => {
        if (dirName === '..') {
            // Go up one level
            const parts = currentPath.split('/').filter((p) => p);
            parts.pop();
            setCurrentPath(parts.length === 0 ? '/' : '/' + parts.join('/'));
        } else {
            // Go into subdirectory
            setCurrentPath(currentPath === '/' ? `/${dirName}` : `${currentPath}/${dirName}`);
        }
    };

    // Format file size
    const formatSize = (bytes: number): string => {
        if (bytes < 1024) {
            return `${bytes} B`;
        }
        if (bytes < 1024 * 1024) {
            return `${(bytes / 1024).toFixed(1)} KB`;
        }
        return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    };

    // Build breadcrumbs for current path
    const pathParts = currentPath.split('/').filter((p) => p);
    const breadcrumbs = [
        <Anchor
            key="root"
            onClick={() => {
                setCurrentPath('/');
            }}
            size="sm"
        >
            {getDriveLetter(driveNumber)}:
        </Anchor>,
        ...pathParts.map((part, idx) => (
            <Anchor
                key={`path-${pathParts.slice(0, idx + 1).join('/')}`}
                onClick={() => {
                    const newPath = '/' + pathParts.slice(0, idx + 1).join('/');
                    setCurrentPath(newPath);
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
                            onChange={(file) => {
                                void uploadFile(file);
                            }}
                            accept="*/*"
                        >
                            {(props) => (
                                <Button {...props} size="sm" disabled={loading} color="blue">
                                    Upload File
                                </Button>
                            )}
                        </FileButton>
                        <Button
                            onClick={() => {
                                browseDisk(driveNumber, currentPath);
                            }}
                            size="sm"
                            disabled={loading}
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
                            {currentPath !== '/' && (
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
                            {files.map((file) => {
                                const fullPath = currentPath === '/' ? `/${file.name}` : `${currentPath}/${file.name}`;
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
                                                            disabled={loading}
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
                                                        disabled={loading}
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

                    {files.length === 0 && !loading && (
                        <div style={{ textAlign: 'center', padding: '2rem', color: '#666' }}>No files found</div>
                    )}
                </Stack>
            </Modal>

            <Modal
                opened={deleteConfirmation !== null}
                onClose={() => {
                    setDeleteConfirmation(null);
                }}
                title="Confirm Deletion"
                size="sm"
            >
                <Stack gap="md">
                    <Text>
                        Are you sure you want to delete <strong>{deleteConfirmation?.name}</strong>?
                    </Text>
                    <Group justify="flex-end">
                        <Button
                            variant="default"
                            onClick={() => {
                                setDeleteConfirmation(null);
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
        </>
    );
}

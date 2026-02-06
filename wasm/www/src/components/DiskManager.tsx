import { useState, useEffect } from 'react';
import { Modal, Tabs, Table, Button, Group, Stack, Select, Breadcrumbs, Anchor, FileButton } from '@mantine/core';
import { Emu86Computer } from '../types/wasm';

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
}

export function DiskManager({ computer, opened, onClose, onStatusUpdate }: DiskManagerProps) {
  const [activeTab, setActiveTab] = useState<string | null>('browse');
  const [currentDrive, setCurrentDrive] = useState<number>(0x80); // Default to C: (0x80)
  const [currentPath, setCurrentPath] = useState<string>('/');
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [loading, setLoading] = useState(false);

  // Drive options
  const driveOptions = [
    { value: '0', label: 'A: (Floppy)' },
    { value: '1', label: 'B: (Floppy)' },
    { value: '128', label: 'C: (Hard Drive)' },
    { value: '129', label: 'D: (Hard Drive)' },
  ];

  // Refresh directory listing when drive or path changes
  useEffect(() => {
    if (opened && computer && activeTab === 'browse') {
      browseDisk(currentDrive, currentPath);
    }
  }, [opened, computer, currentDrive, currentPath, activeTab]);

  // Browse disk directory
  const browseDisk = async (drive: number, path: string) => {
    if (!computer) return;

    setLoading(true);
    try {
      const result = computer.list_directory(drive, path);
      const fileList = result as FileEntry[];

      // Sort files: directories first, then files, both alphabetically (case-insensitive)
      fileList.sort((a, b) => {
        // Directories come before files
        if (a.isDirectory && !b.isDirectory) return -1;
        if (!a.isDirectory && b.isDirectory) return 1;

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
  };

  // Download entire disk image
  const downloadDisk = async (driveType: 'floppy' | 'hdd', driveNumber: number) => {
    if (!computer) return;

    setLoading(true);
    try {
      const data = driveType === 'floppy'
        ? computer.get_floppy_data(driveNumber)
        : computer.get_hard_drive_data(driveNumber - 0x80);

      if (!data) throw new Error('No data returned');

      const blob = new Blob([data], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `drive_${getDriveLetter(driveType === 'floppy' ? driveNumber : driveNumber)}.img`;
      a.click();
      URL.revokeObjectURL(url);
      onStatusUpdate(`Downloaded ${getDriveLetter(driveType === 'floppy' ? driveNumber : driveNumber)}`);
    } catch (e) {
      onStatusUpdate(`Error downloading disk: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  // Download individual file
  const downloadFile = async (drive: number, filePath: string, fileName: string) => {
    if (!computer) return;

    setLoading(true);
    try {
      const data = computer.read_file_from_disk(drive, filePath);
      const blob = new Blob([data], { type: 'application/octet-stream' });
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
  const uploadFile = async (file: File) => {
    if (!computer) return;

    setLoading(true);
    try {
      const arrayBuffer = await file.arrayBuffer();
      const data = new Uint8Array(arrayBuffer);
      const targetPath = currentPath === '/' ? `/${file.name}` : `${currentPath}/${file.name}`;
      computer.write_file_to_disk(currentDrive, targetPath, Array.from(data));
      onStatusUpdate(`Uploaded ${file.name} to ${getDriveLetter(currentDrive)}:${targetPath}`);
      await browseDisk(currentDrive, currentPath); // Refresh listing
    } catch (e) {
      onStatusUpdate(`Error uploading file: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  // Delete file or directory
  const deleteItem = async (drive: number, path: string, name: string) => {
    if (!computer) return;
    if (!confirm(`Are you sure you want to delete ${name}?`)) return;

    setLoading(true);
    try {
      computer.delete_from_disk(drive, path);
      onStatusUpdate(`Deleted ${name}`);
      await browseDisk(currentDrive, currentPath); // Refresh listing
    } catch (e) {
      onStatusUpdate(`Error deleting: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  // Navigate to directory
  const navigateToDirectory = (dirName: string) => {
    if (dirName === '..') {
      // Go up one level
      const parts = currentPath.split('/').filter(p => p);
      parts.pop();
      setCurrentPath(parts.length === 0 ? '/' : '/' + parts.join('/'));
    } else {
      // Go into subdirectory
      setCurrentPath(currentPath === '/' ? `/${dirName}` : `${currentPath}/${dirName}`);
    }
  };

  // Get drive letter from drive number
  const getDriveLetter = (drive: number): string => {
    if (drive === 0) return 'A';
    if (drive === 1) return 'B';
    if (drive >= 0x80) return String.fromCharCode(67 + (drive - 0x80)); // C, D, E, etc.
    return '?';
  };

  // Format file size
  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  // Build breadcrumbs for current path
  const pathParts = currentPath.split('/').filter(p => p);
  const breadcrumbs = [
    <Anchor key="root" onClick={() => setCurrentPath('/')} size="sm">
      {getDriveLetter(currentDrive)}:
    </Anchor>,
    ...pathParts.map((part, idx) => (
      <Anchor
        key={idx}
        onClick={() => {
          const newPath = '/' + pathParts.slice(0, idx + 1).join('/');
          setCurrentPath(newPath);
        }}
        size="sm"
      >
        {part}
      </Anchor>
    ))
  ];

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      size="xl"
      title="Disk Manager"
    >
      <Tabs value={activeTab} onChange={setActiveTab}>
        <Tabs.List>
          <Tabs.Tab value="browse">Browse Files</Tabs.Tab>
          <Tabs.Tab value="download">Download Disk</Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="browse" pt="md">
          <Stack gap="md">
            <Group justify="space-between">
              <Select
                label="Drive"
                value={currentDrive.toString()}
                onChange={(value) => {
                  setCurrentDrive(parseInt(value || '128'));
                  setCurrentPath('/');
                }}
                data={driveOptions}
                style={{ width: 200 }}
              />
              <Group>
                <FileButton onChange={(file) => file && uploadFile(file)} accept="*/*">
                  {(props) => <Button {...props} size="sm" disabled={loading}>Upload File</Button>}
                </FileButton>
                <Button
                  onClick={() => browseDisk(currentDrive, currentPath)}
                  size="sm"
                  disabled={loading}
                >
                  Refresh
                </Button>
              </Group>
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
                      <Anchor onClick={() => navigateToDirectory('..')}>📁 ..</Anchor>
                    </Table.Td>
                    <Table.Td>-</Table.Td>
                    <Table.Td>-</Table.Td>
                    <Table.Td>-</Table.Td>
                  </Table.Tr>
                )}
                {files.map((file, idx) => {
                  const fullPath = currentPath === '/' ? `/${file.name}` : `${currentPath}/${file.name}`;
                  return (
                    <Table.Tr key={idx}>
                      <Table.Td>
                        {file.isDirectory ? (
                          <Anchor onClick={() => navigateToDirectory(file.name)}>
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
                            <Button
                              size="compact-xs"
                              onClick={() => downloadFile(currentDrive, fullPath, file.name)}
                              disabled={loading}
                            >
                              Download
                            </Button>
                          )}
                          <Button
                            size="compact-xs"
                            color="red"
                            onClick={() => deleteItem(currentDrive, fullPath, file.name)}
                            disabled={loading}
                          >
                            Delete
                          </Button>
                        </Group>
                      </Table.Td>
                    </Table.Tr>
                  );
                })}
              </Table.Tbody>
            </Table>

            {files.length === 0 && !loading && (
              <div style={{ textAlign: 'center', padding: '2rem', color: '#666' }}>
                No files found
              </div>
            )}
          </Stack>
        </Tabs.Panel>

        <Tabs.Panel value="download" pt="md">
          <Stack gap="md">
            <div>
              <h3>Download Floppy Drives</h3>
              <Group gap="xs">
                <Button onClick={() => downloadDisk('floppy', 0)} disabled={loading}>
                  Download A:
                </Button>
                <Button onClick={() => downloadDisk('floppy', 1)} disabled={loading}>
                  Download B:
                </Button>
              </Group>
            </div>

            <div>
              <h3>Download Hard Drives</h3>
              <Group gap="xs">
                <Button onClick={() => downloadDisk('hdd', 0x80)} disabled={loading}>
                  Download C:
                </Button>
                <Button onClick={() => downloadDisk('hdd', 0x81)} disabled={loading}>
                  Download D:
                </Button>
              </Group>
            </div>
          </Stack>
        </Tabs.Panel>
      </Tabs>
    </Modal>
  );
}

import { useState, useEffect } from 'react';
import { ActionIcon, Menu, Text, Tooltip } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { fetchImagesLibrary, fetchImageData, DiskImage } from '../imageLibrary';
import { status } from '../emulatorState';

type DriveType = 'floppy' | 'hdd' | 'cdrom';

interface ImageLibraryPickerProps {
    driveType: DriveType;
    onLoad: (data: Uint8Array, name: string) => void;
}

export function ImageLibraryPicker({ driveType, onLoad }: ImageLibraryPickerProps): React.ReactElement | null {
    const [images, setImages] = useState<DiskImage[]>([]);
    const [loading, setLoading] = useState(false);

    useEffect(() => {
        void fetchImagesLibrary().then((lib) => {
            setImages(lib[driveType]);
        });
    }, [driveType]);

    if (images.length === 0) {
        return null;
    }

    const handleSelect = async (image: DiskImage): Promise<void> => {
        try {
            setLoading(true);
            status.value = `Fetching ${image.name}...`;
            const data = await fetchImageData(image.url);
            onLoad(data, image.name);
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            status.value = `Error fetching ${image.name}: ${msg}`;
            notifications.show({
                color: 'red',
                title: `Failed to load "${image.name}"`,
                message: msg,
            });
            console.error(e);
        } finally {
            setLoading(false);
        }
    };

    return (
        <Menu shadow="md" width={260}>
            <Menu.Target>
                <Tooltip label="Select from server library">
                    <ActionIcon size="md" variant="default" loading={loading}>
                        <i className="bi bi-server"></i>
                    </ActionIcon>
                </Tooltip>
            </Menu.Target>
            <Menu.Dropdown>
                <Menu.Label>Server Images</Menu.Label>
                {images.map((img) => (
                    <Menu.Item
                        key={img.url}
                        onClick={() => {
                            void handleSelect(img);
                        }}
                    >
                        <Text size="sm" fw={500}>
                            {img.name}
                        </Text>
                        {img.description && (
                            <Text size="xs" c="dimmed">
                                {img.description}
                            </Text>
                        )}
                    </Menu.Item>
                ))}
            </Menu.Dropdown>
        </Menu>
    );
}

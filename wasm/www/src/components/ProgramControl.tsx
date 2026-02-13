import { Group, Button, Text, FileButton, TextInput, Stack } from '@mantine/core';
import { useState } from 'react';

interface ProgramControlProps {
    onLoadProgram: (file: File, segment: number, offset: number) => void;
    onReset: () => void;
}

export function ProgramControl({ onLoadProgram, onReset }: ProgramControlProps): React.ReactElement {
    const [file, setFile] = useState<File | null>(null);
    const [segment, setSegment] = useState('0x0000');
    const [offset, setOffset] = useState('0x0100');

    const parseHex = (value: string): number | null => {
        const cleaned = value.trim();
        const parsed = parseInt(cleaned, 16);
        return isNaN(parsed) || parsed < 0 || parsed > 0xffff ? null : parsed;
    };

    const handleLoad = (): void => {
        if (!file) {
            return;
        }

        const segmentValue = parseHex(segment);
        const offsetValue = parseHex(offset);

        if (segmentValue === null || offsetValue === null) {
            alert('Invalid segment or offset. Use hex format (e.g., 0x0000)');
            return;
        }

        onLoadProgram(file, segmentValue, offsetValue);
    };

    return (
        <Stack gap="xs" p="sm" style={{ border: '1px solid var(--mantine-color-dark-4)', borderRadius: 4 }}>
            <Group gap="xs" align="center">
                <Text fw={500} size="sm" style={{ minWidth: 100, textAlign: 'right' }}>
                    Program File:
                </Text>
                <FileButton onChange={setFile} accept=".com,.bin,.exe">
                    {(props) => (
                        <Button {...props} size="compact-sm" color="blue">
                            {file ? file.name : 'Choose File'}
                        </Button>
                    )}
                </FileButton>
            </Group>

            <Group gap="xs" align="center">
                <Text fw={500} size="sm" style={{ minWidth: 100, textAlign: 'right' }}>
                    Segment:
                </Text>
                <TextInput
                    placeholder="0x0000"
                    value={segment}
                    onChange={(e) => {
                        setSegment(e.currentTarget.value);
                    }}
                    size="xs"
                    style={{ width: 100 }}
                />
            </Group>

            <Group gap="xs" align="center">
                <Text fw={500} size="sm" style={{ minWidth: 100, textAlign: 'right' }}>
                    Offset:
                </Text>
                <TextInput
                    placeholder="0x0100"
                    value={offset}
                    onChange={(e) => {
                        setOffset(e.currentTarget.value);
                    }}
                    size="xs"
                    style={{ width: 100 }}
                />
            </Group>

            <Group gap="xs" mt="xs">
                <Button onClick={handleLoad} disabled={!file} color="green" size="compact-sm" style={{ flex: 1 }}>
                    Load Program
                </Button>
                <Button onClick={onReset} color="red" size="compact-sm" style={{ flex: 1 }}>
                    Reset
                </Button>
            </Group>
        </Stack>
    );
}

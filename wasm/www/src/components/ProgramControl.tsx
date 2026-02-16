import { useSignal } from '@preact/signals-react';
import { Group, Button, Text, FileButton, TextInput, Stack } from '@mantine/core';

interface ProgramControlProps {
    onLoadProgram: (file: File, segment: number, offset: number) => void;
}

export function ProgramControl({ onLoadProgram }: ProgramControlProps): React.ReactElement {
    const file = useSignal<File | null>(null);
    const segment = useSignal('0x0000');
    const offset = useSignal('0x0100');

    const parseHex = (value: string): number | null => {
        const cleaned = value.trim();
        const parsed = parseInt(cleaned, 16);
        return isNaN(parsed) || parsed < 0 || parsed > 0xffff ? null : parsed;
    };

    const handleLoad = (): void => {
        if (!file.value) {
            return;
        }

        const segmentValue = parseHex(segment.value);
        const offsetValue = parseHex(offset.value);

        if (segmentValue === null || offsetValue === null) {
            alert('Invalid segment or offset. Use hex format (e.g., 0x0000)');
            return;
        }

        onLoadProgram(file.value, segmentValue, offsetValue);
    };

    return (
        <Stack gap="xs" p="sm" style={{ border: '1px solid var(--mantine-color-dark-4)', borderRadius: 4 }}>
            <Group gap="xs" align="center">
                <Text fw={500} size="sm" style={{ minWidth: 100, textAlign: 'right' }}>
                    Program File:
                </Text>
                <FileButton
                    onChange={(f) => {
                        file.value = f;
                    }}
                    accept=".com,.bin,.exe"
                >
                    {(props) => (
                        <Button {...props} size="compact-sm" color="blue">
                            {file.value ? file.value.name : 'Choose File'}
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
                    value={segment.value}
                    onChange={(e) => {
                        segment.value = e.currentTarget.value;
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
                    value={offset.value}
                    onChange={(e) => {
                        offset.value = e.currentTarget.value;
                    }}
                    size="xs"
                    style={{ width: 100 }}
                />
            </Group>

            <Group gap="xs" mt="xs">
                <Button onClick={handleLoad} disabled={!file.value} color="green" size="compact-sm" style={{ flex: 1 }}>
                    Load Program
                </Button>
            </Group>
        </Stack>
    );
}

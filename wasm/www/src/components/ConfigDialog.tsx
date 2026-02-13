import { useState } from 'react';
import { Modal, Stack, Select, Button, Group, Text, Alert } from '@mantine/core';
import { CLOCK_OPTIONS, CPU_OPTIONS, EmulatorConfig, MEMORY_OPTIONS, VIDEO_CARD_OPTIONS } from './ConfigDialog.consts';

interface ConfigDialogProps {
    opened: boolean;
    onClose: () => void;
    currentConfig: EmulatorConfig;
    onApply: (config: EmulatorConfig) => void;
    isRunning: boolean;
}

function ConfigForm({
    onClose,
    currentConfig,
    onApply,
    isRunning,
}: Omit<ConfigDialogProps, 'opened'>): React.ReactElement {
    const [cpuType, setCpuType] = useState(currentConfig.cpuType);
    const [memoryKb, setMemoryKb] = useState(String(currentConfig.memoryKb));
    const [clockMhz, setClockMhz] = useState(String(currentConfig.clockMhz));
    const [videoCard, setVideoCard] = useState(currentConfig.videoCard);

    const handleApply = (): void => {
        onApply({
            cpuType,
            memoryKb: parseInt(memoryKb, 10),
            clockMhz: parseFloat(clockMhz),
            videoCard,
        });
        onClose();
    };

    const needsExtendedRam = parseInt(memoryKb, 10) > 640;

    return (
        <Stack gap="md">
            {isRunning && (
                <Alert color="yellow" title="Warning">
                    Applying configuration will reset the emulator and stop execution.
                </Alert>
            )}

            <div>
                <Text size="sm" fw={500} mb={4}>
                    CPU Type
                </Text>
                <Select
                    data={CPU_OPTIONS}
                    value={cpuType}
                    onChange={(v) => {
                        if (v) {
                            setCpuType(v);
                        }
                    }}
                />
            </div>

            <div>
                <Text size="sm" fw={500} mb={4}>
                    Memory
                </Text>
                <Select
                    data={MEMORY_OPTIONS}
                    value={memoryKb}
                    onChange={(v) => {
                        if (v) {
                            setMemoryKb(v);
                        }
                    }}
                />
                {needsExtendedRam && cpuType === '8086' && (
                    <Text size="xs" c="red" mt={2}>
                        Extended memory requires 286 or later CPU
                    </Text>
                )}
            </div>

            <div>
                <Text size="sm" fw={500} mb={4}>
                    Clock Speed
                </Text>
                <Select
                    data={CLOCK_OPTIONS}
                    value={clockMhz}
                    onChange={(v) => {
                        if (v) {
                            setClockMhz(v);
                        }
                    }}
                />
            </div>

            <div>
                <Text size="sm" fw={500} mb={4}>
                    Video Card
                </Text>
                <Select
                    data={VIDEO_CARD_OPTIONS}
                    value={videoCard}
                    onChange={(v) => {
                        if (v) {
                            setVideoCard(v);
                        }
                    }}
                />
            </div>

            <Group justify="flex-end" gap="xs">
                <Button variant="default" onClick={onClose}>
                    Cancel
                </Button>
                <Button onClick={handleApply}>Apply &amp; Reset</Button>
            </Group>
        </Stack>
    );
}

export function ConfigDialog({
    opened,
    onClose,
    currentConfig,
    onApply,
    isRunning,
}: ConfigDialogProps): React.ReactElement {
    return (
        <Modal opened={opened} onClose={onClose} title="System Configuration" size="sm">
            <ConfigForm
                key={String(opened)}
                onClose={onClose}
                currentConfig={currentConfig}
                onApply={onApply}
                isRunning={isRunning}
            />
        </Modal>
    );
}

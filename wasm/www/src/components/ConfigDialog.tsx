import { useSignal, useSignalEffect } from '@preact/signals-react';
import { Modal, Stack, Select, Button, Group, Text, Alert, Grid, Checkbox, Badge } from '@mantine/core';
import {
    CLOCK_OPTIONS,
    COM_PORT_OPTIONS,
    CPU_OPTIONS,
    EmulatorConfig,
    MEMORY_OPTIONS,
    VIDEO_CARD_OPTIONS,
} from './ConfigDialog.consts';

interface ConfigDialogProps {
    opened: boolean;
    onClose: () => void;
    currentConfig: EmulatorConfig;
    onApply: (config: EmulatorConfig) => void;
    isRunning: boolean;
    joystickConnected: [boolean, boolean];
}

function ConfigForm({
    onClose,
    currentConfig,
    onApply,
    isRunning,
    joystickConnected,
}: Omit<ConfigDialogProps, 'opened'>): React.ReactElement {
    const cpuType = useSignal(currentConfig.cpuType);
    const memoryKb = useSignal(String(currentConfig.memoryKb));
    const clockMhz = useSignal(String(currentConfig.clockMhz));
    const videoCard = useSignal(currentConfig.videoCard);
    const com1Device = useSignal(currentConfig.com1Device);
    const com2Device = useSignal(currentConfig.com2Device);
    const joystickA = useSignal(currentConfig.joystickA);
    const joystickB = useSignal(currentConfig.joystickB);
    const physicalGamepads = useSignal<number>(0);

    // Poll for physical gamepads while dialog is open
    useSignalEffect(() => {
        const poll = (): void => {
            physicalGamepads.value = navigator.getGamepads().filter(Boolean).length;
        };
        poll();
        const id = setInterval(poll, 500);
        return () => {
            clearInterval(id);
        };
    });

    const handleApply = (): void => {
        onApply({
            cpuType: cpuType.value,
            memoryKb: parseInt(memoryKb.value, 10),
            clockMhz: parseFloat(clockMhz.value),
            videoCard: videoCard.value,
            com1Device: com1Device.value,
            com2Device: com2Device.value,
            joystickA: joystickA.value,
            joystickB: joystickB.value,
        });
        onClose();
    };

    const needsExtendedRam = parseInt(memoryKb.value, 10) > 640;

    return (
        <Stack gap="md">
            {isRunning && (
                <Alert color="yellow" title="Warning">
                    Applying configuration will reset the emulator and stop execution.
                </Alert>
            )}

            <Grid gutter="md">
                <Grid.Col span={6}>
                    <div>
                        <Text size="sm" fw={500} mb={4}>
                            CPU Type
                        </Text>
                        <Select
                            data={CPU_OPTIONS}
                            value={cpuType.value}
                            onChange={(v) => {
                                if (v) {
                                    cpuType.value = v;
                                }
                            }}
                        />
                    </div>
                </Grid.Col>

                <Grid.Col span={6}>
                    <div>
                        <Text size="sm" fw={500} mb={4}>
                            Video Card
                        </Text>
                        <Select
                            data={VIDEO_CARD_OPTIONS}
                            value={videoCard.value}
                            onChange={(v) => {
                                if (v) {
                                    videoCard.value = v;
                                }
                            }}
                        />
                    </div>
                </Grid.Col>

                <Grid.Col span={6}>
                    <div>
                        <Text size="sm" fw={500} mb={4}>
                            Memory
                        </Text>
                        <Select
                            data={MEMORY_OPTIONS}
                            value={memoryKb.value}
                            onChange={(v) => {
                                if (v) {
                                    memoryKb.value = v;
                                }
                            }}
                        />
                        {needsExtendedRam && cpuType.value === '8086' && (
                            <Text size="xs" c="red" mt={2}>
                                Extended memory requires 286 or later CPU
                            </Text>
                        )}
                    </div>
                </Grid.Col>

                <Grid.Col span={6}>
                    <div>
                        <Text size="sm" fw={500} mb={4}>
                            COM1 Device
                        </Text>
                        <Select
                            data={COM_PORT_OPTIONS}
                            value={com1Device.value}
                            onChange={(v) => {
                                if (v) {
                                    com1Device.value = v;
                                }
                            }}
                        />
                    </div>
                </Grid.Col>

                <Grid.Col span={6}>
                    <div>
                        <Text size="sm" fw={500} mb={4}>
                            Clock Speed
                        </Text>
                        <Select
                            data={CLOCK_OPTIONS}
                            value={clockMhz.value}
                            onChange={(v) => {
                                if (v) {
                                    clockMhz.value = v;
                                }
                            }}
                        />
                    </div>
                </Grid.Col>

                <Grid.Col span={6}>
                    <div>
                        <Text size="sm" fw={500} mb={4}>
                            COM2 Device
                        </Text>
                        <Select
                            data={COM_PORT_OPTIONS}
                            value={com2Device.value}
                            onChange={(v) => {
                                if (v) {
                                    com2Device.value = v;
                                }
                            }}
                        />
                    </div>
                </Grid.Col>

                <Grid.Col span={6}>
                    <div>
                        <Group gap="xs" mb={4}>
                            <Text size="sm" fw={500}>
                                Joystick
                            </Text>
                            <Badge size="xs" color={physicalGamepads.value > 0 ? 'green' : 'gray'} variant="light">
                                {physicalGamepads.value > 0
                                    ? `${physicalGamepads.value} gamepad${physicalGamepads.value > 1 ? 's' : ''} detected`
                                    : 'no gamepad'}
                            </Badge>
                        </Group>
                        <Stack gap="xs">
                            <Checkbox
                                label={
                                    <Group gap="xs" wrap="nowrap" align="center">
                                        <span>Joystick A</span>
                                        {joystickA.value && (
                                            <Badge size="xs" color={joystickConnected[0] ? 'green' : 'orange'} variant="dot">
                                                {joystickConnected[0] ? 'active' : 'pending reset'}
                                            </Badge>
                                        )}
                                    </Group>
                                }
                                checked={joystickA.value}
                                onChange={(e) => {
                                    joystickA.value = e.currentTarget.checked;
                                }}
                            />
                            <Checkbox
                                label={
                                    <Group gap="xs" wrap="nowrap" align="center">
                                        <span>Joystick B</span>
                                        {joystickB.value && (
                                            <Badge size="xs" color={joystickConnected[1] ? 'green' : 'orange'} variant="dot">
                                                {joystickConnected[1] ? 'active' : 'pending reset'}
                                            </Badge>
                                        )}
                                    </Group>
                                }
                                checked={joystickB.value}
                                onChange={(e) => {
                                    joystickB.value = e.currentTarget.checked;
                                }}
                            />
                        </Stack>
                    </div>
                </Grid.Col>
            </Grid>

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
    joystickConnected,
}: ConfigDialogProps): React.ReactElement {
    return (
        <Modal opened={opened} onClose={onClose} title="System Configuration" size="lg">
            <ConfigForm
                key={String(opened)}
                onClose={onClose}
                currentConfig={currentConfig}
                onApply={onApply}
                isRunning={isRunning}
                joystickConnected={joystickConnected}
            />
        </Modal>
    );
}

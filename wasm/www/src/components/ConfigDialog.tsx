import { useState, useEffect } from 'react';
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
    const [cpuType, setCpuType] = useState(currentConfig.cpuType);
    const [memoryKb, setMemoryKb] = useState(String(currentConfig.memoryKb));
    const [clockMhz, setClockMhz] = useState(String(currentConfig.clockMhz));
    const [videoCard, setVideoCard] = useState(currentConfig.videoCard);
    const [com1Device, setCom1Device] = useState(currentConfig.com1Device);
    const [com2Device, setCom2Device] = useState(currentConfig.com2Device);
    const [joystickA, setJoystickA] = useState(currentConfig.joystickA);
    const [joystickB, setJoystickB] = useState(currentConfig.joystickB);
    const [physicalGamepads, setPhysicalGamepads] = useState<number>(0);

    // Poll for physical gamepads while dialog is open
    useEffect(() => {
        const poll = (): void => {
            const count = navigator.getGamepads().filter(Boolean).length;
            setPhysicalGamepads(count);
        };
        poll();
        const id = setInterval(poll, 500);
        return () => {
            clearInterval(id);
        };
    }, []);

    const handleApply = (): void => {
        onApply({
            cpuType,
            memoryKb: parseInt(memoryKb, 10),
            clockMhz: parseFloat(clockMhz),
            videoCard,
            com1Device,
            com2Device,
            joystickA,
            joystickB,
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

            <Grid gutter="md">
                <Grid.Col span={6}>
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
                </Grid.Col>

                <Grid.Col span={6}>
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
                </Grid.Col>

                <Grid.Col span={6}>
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
                </Grid.Col>

                <Grid.Col span={6}>
                    <div>
                        <Text size="sm" fw={500} mb={4}>
                            COM1 Device
                        </Text>
                        <Select
                            data={COM_PORT_OPTIONS}
                            value={com1Device}
                            onChange={(v) => {
                                if (v) {
                                    setCom1Device(v);
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
                            value={clockMhz}
                            onChange={(v) => {
                                if (v) {
                                    setClockMhz(v);
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
                            value={com2Device}
                            onChange={(v) => {
                                if (v) {
                                    setCom2Device(v);
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
                            <Badge size="xs" color={physicalGamepads > 0 ? 'green' : 'gray'} variant="light">
                                {physicalGamepads > 0
                                    ? `${physicalGamepads} gamepad${physicalGamepads > 1 ? 's' : ''} detected`
                                    : 'no gamepad'}
                            </Badge>
                        </Group>
                        <Stack gap="xs">
                            <Group gap="xs">
                                <Checkbox
                                    label="Joystick A (gamepad 1)"
                                    checked={joystickA}
                                    onChange={(e) => {
                                        setJoystickA(e.currentTarget.checked);
                                    }}
                                />
                                {joystickA && (
                                    <Badge size="xs" color={joystickConnected[0] ? 'green' : 'orange'} variant="dot">
                                        {joystickConnected[0] ? 'active' : 'pending reset'}
                                    </Badge>
                                )}
                            </Group>
                            <Group gap="xs">
                                <Checkbox
                                    label="Joystick B (gamepad 2)"
                                    checked={joystickB}
                                    onChange={(e) => {
                                        setJoystickB(e.currentTarget.checked);
                                    }}
                                />
                                {joystickB && (
                                    <Badge size="xs" color={joystickConnected[1] ? 'green' : 'orange'} variant="dot">
                                        {joystickConnected[1] ? 'active' : 'pending reset'}
                                    </Badge>
                                )}
                            </Group>
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

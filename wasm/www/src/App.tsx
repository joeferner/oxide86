import React, { useEffect, useRef } from 'react';
import { useSignal, useSignalEffect } from '@preact/signals-react';
import { Container, Title, Group, Paper, Stack, SegmentedControl, Button } from '@mantine/core';
import {
    computer,
    status,
    isRunning,
    perfStats,
    joystickConnected,
    config,
    bootDrive,
    initEmulator,
    startExecution,
    stopExecution,
    bootAndStart,
    loadProgram,
    reset,
    applyConfig,
} from './emulatorState';
import { isLocked } from './pointerLockState';
import { EmulatorCanvas } from './components/EmulatorCanvas';
import { InfoBox } from './components/InfoBox';
import { DriveControl } from './components/DriveControl';
import { BootControl } from './components/BootControl';
import { ProgramControl } from './components/ProgramControl';
import { ExecutionControl } from './components/ExecutionControl';
import { StatusDisplay } from './components/StatusDisplay';
import { RunningIndicator } from './components/RunningIndicator';
import { PerformanceDisplay } from './components/PerformanceDisplay';
import { DiskManager } from './components/DiskManager';
import { ConfigDialog } from './components/ConfigDialog';
import { saveConfig, EmulatorConfig } from './components/ConfigDialog.consts';

function App(): React.ReactElement {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const mode = useSignal<'boot' | 'program'>('boot');
    const diskManagerOpened = useSignal(false);
    const configOpened = useSignal(false);
    const selectedDrive = useSignal<number>(0x80);
    const hasBooted = useSignal(false);
    const floppyLabels = useSignal<[string | null, string | null]>([null, null]);

    // Initialize emulator once canvas is available
    useEffect(() => {
        let mounted = true;
        const init = async (): Promise<void> => {
            if (!canvasRef.current) {
                if (mounted) {
                    setTimeout(() => void init(), 100);
                }
                return;
            }
            await initEmulator(canvasRef.current);
        };
        void init();
        return () => {
            mounted = false;
        };
    }, []);

    // Warn before unload when session is active
    useSignalEffect(() => {
        if (!hasBooted.value) {
            return;
        }
        const handler = (e: BeforeUnloadEvent): void => {
            e.preventDefault();
        };
        window.addEventListener('beforeunload', handler);
        return () => {
            window.removeEventListener('beforeunload', handler);
        };
    });

    const handleAction = (): void => {
        if (!hasBooted.value) {
            if (mode.value === 'boot') {
                bootAndStart();
            } else {
                startExecution();
            }
            hasBooted.value = true;
        } else if (isRunning.value) {
            stopExecution();
        } else {
            startExecution();
        }
    };

    const handleReset = (): void => {
        reset();
        hasBooted.value = false;
    };

    const handleLoadProgram = async (file: File, segment: number, offset: number): Promise<void> => {
        try {
            const arrayBuffer = await file.arrayBuffer();
            loadProgram(new Uint8Array(arrayBuffer), segment, offset);
        } catch (e) {
            status.value = `Failed to load file: ${e}`;
            console.error(e);
        }
    };

    const handleApplyConfig = (cfg: EmulatorConfig): void => {
        saveConfig(cfg);
        applyConfig(cfg);
        hasBooted.value = false;
    };

    return (
        <Container size="xl" p="md">
            <Group justify="space-between" mb="md">
                <Title order={1}>emu86 - Intel 8086 Emulator</Title>
                <Button
                    variant="default"
                    leftSection="⚙"
                    onClick={() => {
                        configOpened.value = true;
                    }}
                >
                    System Configuration
                </Button>
            </Group>

            <InfoBox isPointerLocked={isLocked.value} />

            <Group align="flex-start" gap="md" mt="md" wrap="nowrap">
                <Stack gap="xs">
                    <EmulatorCanvas ref={canvasRef} computer={computer.value} />
                    <Group gap="md" grow>
                        <RunningIndicator isRunning={isRunning.value} />
                        <PerformanceDisplay performance={perfStats.value} />
                    </Group>
                </Stack>

                <Paper shadow="sm" p="md" style={{ flex: 1, minWidth: 300 }} withBorder>
                    <Stack gap="xs">
                        <DriveControl
                            onManageDrive={(driveNumber) => {
                                selectedDrive.value = driveNumber;
                                diskManagerOpened.value = true;
                            }}
                            floppyALabel={floppyLabels.value[0]}
                            floppyBLabel={floppyLabels.value[1]}
                            onFloppyEjected={(slot) => {
                                const next: [string | null, string | null] = [
                                    floppyLabels.value[0],
                                    floppyLabels.value[1],
                                ];
                                next[slot] = null;
                                floppyLabels.value = next;
                            }}
                        />

                        <SegmentedControl
                            value={mode.value}
                            onChange={(value) => {
                                mode.value = value as 'boot' | 'program';
                            }}
                            data={[
                                { label: 'Boot from Disk', value: 'boot' },
                                { label: 'Load Program', value: 'program' },
                            ]}
                            fullWidth
                            mb="xs"
                        />

                        {mode.value === 'boot' ? (
                            <BootControl
                                onBootA={() => {
                                    bootDrive.value = 0x00;
                                }}
                                onBootC={() => {
                                    bootDrive.value = 0x80;
                                }}
                                bootDrive={bootDrive.value}
                            />
                        ) : (
                            <ProgramControl
                                onLoadProgram={(file, segment, offset) => {
                                    void handleLoadProgram(file, segment, offset);
                                }}
                            />
                        )}

                        <ExecutionControl
                            mode={mode.value}
                            isRunning={isRunning.value}
                            hasBooted={hasBooted.value}
                            onAction={handleAction}
                            onReset={handleReset}
                        />

                        <StatusDisplay status={status.value} />
                    </Stack>
                </Paper>
            </Group>

            <DiskManager
                opened={diskManagerOpened.value}
                onClose={() => {
                    diskManagerOpened.value = false;
                }}
                driveNumber={selectedDrive.value}
                onFloppyCreated={(slot, label) => {
                    const next: [string | null, string | null] = [floppyLabels.value[0], floppyLabels.value[1]];
                    next[slot] = label;
                    floppyLabels.value = next;
                }}
            />

            <ConfigDialog
                opened={configOpened.value}
                onClose={() => {
                    configOpened.value = false;
                }}
                currentConfig={config.value}
                onApply={handleApplyConfig}
                isRunning={isRunning.value}
                joystickConnected={joystickConnected.value}
            />
        </Container>
    );
}

export default App;

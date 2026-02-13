import React, { useCallback, useRef, useState } from 'react';
import { Container, Title, Group, Paper, Stack, SegmentedControl, Button } from '@mantine/core';
import { useEmulator } from './hooks/useEmulator';
import { usePointerLock } from './hooks/usePointerLock';
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
import { DEFAULT_CONFIG, EmulatorConfig } from './components/ConfigDialog.consts';

function App(): React.ReactElement {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const { isLocked } = usePointerLock(canvasRef);
    const [mode, setMode] = useState<'boot' | 'program'>('boot');
    const [diskManagerOpened, setDiskManagerOpened] = useState(false);
    const [configOpened, setConfigOpened] = useState(false);
    const [currentConfig, setCurrentConfig] = useState<EmulatorConfig>(DEFAULT_CONFIG);
    const [selectedDrive, setSelectedDrive] = useState<number>(0x80);
    const [bootDrive, setBootDrive] = useState<number>(0x80); // Default to C:

    const {
        computer,
        status,
        setStatus,
        isRunning,
        performance,
        startExecution,
        stopExecution,
        stepExecution,
        loadProgram,
        reset,
        bootAndStart,
        applyConfig,
    } = useEmulator(canvasRef, bootDrive);

    const handleStatusUpdate = useCallback(
        (message: string) => {
            setStatus(message);
        },
        [setStatus]
    );

    const handleBootA = useCallback(() => {
        setBootDrive(0x00);
    }, []);

    const handleBootC = useCallback(() => {
        setBootDrive(0x80);
    }, []);

    const handleLoadProgram = useCallback(
        async (file: File, segment: number, offset: number) => {
            try {
                const arrayBuffer = await file.arrayBuffer();
                const data = new Uint8Array(arrayBuffer);
                loadProgram(data, segment, offset);
            } catch (e) {
                setStatus(`Failed to load file: ${e}`);
                console.error(e);
            }
        },
        [loadProgram, setStatus]
    );

    const handleManageDrive = useCallback((driveNumber: number) => {
        setSelectedDrive(driveNumber);
        setDiskManagerOpened(true);
    }, []);

    const handleApplyConfig = useCallback(
        (config: EmulatorConfig) => {
            setCurrentConfig(config);
            applyConfig(config);
        },
        [applyConfig]
    );

    return (
        <Container size="xl" p="md">
            <Group justify="space-between" mb="md">
                <Title order={1}>emu86 - Intel 8086 Emulator</Title>
                <Button
                    variant="default"
                    leftSection="⚙"
                    onClick={() => {
                        setConfigOpened(true);
                    }}
                >
                    System Configuration
                </Button>
            </Group>

            <InfoBox isPointerLocked={isLocked} />

            <Group align="flex-start" gap="md" mt="md" wrap="nowrap">
                <Stack gap="xs">
                    <EmulatorCanvas ref={canvasRef} computer={computer} onStatusUpdate={handleStatusUpdate} />
                    <Group gap="md" grow>
                        <RunningIndicator isRunning={isRunning} />
                        <PerformanceDisplay performance={performance} />
                    </Group>
                </Stack>

                <Paper shadow="sm" p="md" style={{ flex: 1, minWidth: 300 }} withBorder>
                    <Stack gap="xs">
                        <DriveControl
                            computer={computer}
                            onStatusUpdate={handleStatusUpdate}
                            onManageDrive={handleManageDrive}
                        />

                        <SegmentedControl
                            value={mode}
                            onChange={(value) => {
                                setMode(value as 'boot' | 'program');
                            }}
                            data={[
                                { label: 'Boot from Disk', value: 'boot' },
                                { label: 'Load Program', value: 'program' },
                            ]}
                            fullWidth
                            mb="xs"
                        />

                        {mode === 'boot' ? (
                            <BootControl
                                onBootA={handleBootA}
                                onBootC={handleBootC}
                                onReset={reset}
                                bootDrive={bootDrive}
                            />
                        ) : (
                            <ProgramControl
                                onLoadProgram={(file, segment, offset) => {
                                    void handleLoadProgram(file, segment, offset);
                                }}
                                onReset={reset}
                            />
                        )}

                        <ExecutionControl
                            isRunning={isRunning}
                            onStart={mode === 'boot' ? bootAndStart : startExecution}
                            onStop={stopExecution}
                            onStep={stepExecution}
                        />

                        <StatusDisplay status={status} />
                    </Stack>
                </Paper>
            </Group>

            <DiskManager
                computer={computer}
                opened={diskManagerOpened}
                onClose={() => {
                    setDiskManagerOpened(false);
                }}
                onStatusUpdate={handleStatusUpdate}
                driveNumber={selectedDrive}
            />

            <ConfigDialog
                opened={configOpened}
                onClose={() => {
                    setConfigOpened(false);
                }}
                currentConfig={currentConfig}
                onApply={handleApplyConfig}
                isRunning={isRunning}
            />
        </Container>
    );
}

export default App;

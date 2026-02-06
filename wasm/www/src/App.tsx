import { useCallback, useRef, useState } from 'react'
import { Container, Title, Group, Paper, Stack, SegmentedControl } from '@mantine/core'
import { useEmulator } from './hooks/useEmulator'
import { usePointerLock } from './hooks/usePointerLock'
import { EmulatorCanvas } from './components/EmulatorCanvas'
import { InfoBox } from './components/InfoBox'
import { DriveControl } from './components/DriveControl'
import { BootControl } from './components/BootControl'
import { ProgramControl } from './components/ProgramControl'
import { ExecutionControl } from './components/ExecutionControl'
import { StatusDisplay } from './components/StatusDisplay'
import { RunningIndicator } from './components/RunningIndicator'
import { PerformanceDisplay } from './components/PerformanceDisplay'
import { DiskManager } from './components/DiskManager'

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { isLocked } = usePointerLock(canvasRef)
  const [mode, setMode] = useState<'boot' | 'program'>('boot')
  const [diskManagerOpened, setDiskManagerOpened] = useState(false)
  const [selectedDrive, setSelectedDrive] = useState<number>(0x80)
  const [bootDrive, setBootDrive] = useState<number>(0x80) // Default to C:

  const {
    computer,
    status,
    setStatus,
    isRunning,
    performance,
    startExecution,
    stopExecution,
    stepExecution,
    boot,
    loadProgram,
    reset,
  } = useEmulator(canvasRef)

  const handleStatusUpdate = useCallback((message: string) => {
    setStatus(message)
  }, [setStatus])

  const handleBootA = useCallback(() => {
    setBootDrive(0x00)
    boot(0x00)
  }, [boot])

  const handleBootC = useCallback(() => {
    setBootDrive(0x80)
    boot(0x80)
  }, [boot])

  const handleLoadProgram = useCallback(async (file: File, segment: number, offset: number) => {
    try {
      const arrayBuffer = await file.arrayBuffer()
      const data = new Uint8Array(arrayBuffer)
      loadProgram(data, segment, offset)
    } catch (e) {
      setStatus(`Failed to load file: ${e}`)
      console.error(e)
    }
  }, [loadProgram, setStatus])

  const handleManageDrive = useCallback((driveNumber: number) => {
    setSelectedDrive(driveNumber)
    setDiskManagerOpened(true)
  }, [])

  return (
    <Container size="xl" p="md">
      <Title order={1} ta="center" mb="md">emu86 - Intel 8086 Emulator</Title>

      <InfoBox isPointerLocked={isLocked} />

      <Group align="flex-start" gap="md" mt="md" wrap="nowrap">
        <Stack gap="xs">
          <EmulatorCanvas
            ref={canvasRef}
            computer={computer}
            onStatusUpdate={handleStatusUpdate}
          />
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
              onChange={(value) => setMode(value as 'boot' | 'program')}
              data={[
                { label: 'Boot from Disk', value: 'boot' },
                { label: 'Load Program', value: 'program' }
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
                onLoadProgram={handleLoadProgram}
                onReset={reset}
              />
            )}

            <ExecutionControl
              isRunning={isRunning}
              onStart={startExecution}
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
        onClose={() => setDiskManagerOpened(false)}
        onStatusUpdate={handleStatusUpdate}
        driveNumber={selectedDrive}
      />
    </Container>
  )
}

export default App

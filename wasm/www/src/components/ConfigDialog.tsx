import { useState, useEffect } from 'react';
import { Modal, Stack, Select, Button, Group, Text, Alert } from '@mantine/core';

export interface EmulatorConfig {
  cpuType: string;
  memoryKb: number;
  clockMhz: number;
  videoCard: string;
}

export const DEFAULT_CONFIG: EmulatorConfig = {
  cpuType: '8086',
  memoryKb: 640,
  clockMhz: 4.77,
  videoCard: 'ega',
};

const CPU_OPTIONS = [
  { value: '8086', label: '8086 (original, 1 MB)' },
  { value: '286', label: '286 (16 MB)' },
  { value: '386', label: '386 (64 MB)' },
  { value: '486', label: '486 (64 MB)' },
];

// Total memory options: conventional (≤640KB) or extended (>640KB, requires 286+)
const MEMORY_OPTIONS = [
  { value: '256', label: '256 KB (conventional)' },
  { value: '512', label: '512 KB (conventional)' },
  { value: '640', label: '640 KB (conventional, standard)' },
  { value: '1024', label: '1 MB (no extended)' },
  { value: '2048', label: '2 MB (1 MB extended, 286+)' },
  { value: '4096', label: '4 MB (3 MB extended, 286+)' },
  { value: '8192', label: '8 MB (7 MB extended, 286+)' },
  { value: '16384', label: '16 MB (15 MB extended, 286+)' },
];

const CLOCK_OPTIONS = [
  { value: '4.77', label: '4.77 MHz (IBM PC/XT, 8088)' },
  { value: '8', label: '8 MHz (IBM PC/AT, 286)' },
  { value: '10', label: '10 MHz (PC/AT 10 MHz, 286)' },
  { value: '12', label: '12 MHz (286)' },
  { value: '16', label: '16 MHz (386SX)' },
  { value: '25', label: '25 MHz (386DX / 486SX)' },
  { value: '33', label: '33 MHz (486DX)' },
  { value: '100', label: '100 MHz (Pentium)' },
];

const VIDEO_CARD_OPTIONS = [
  { value: 'cga', label: 'CGA (text + 4-color graphics)' },
  { value: 'ega', label: 'EGA (CGA + 16-color graphics)' },
  { value: 'vga', label: 'VGA (EGA + VGA modes)' },
];

interface ConfigDialogProps {
  opened: boolean;
  onClose: () => void;
  currentConfig: EmulatorConfig;
  onApply: (config: EmulatorConfig) => void;
  isRunning: boolean;
}

export function ConfigDialog({ opened, onClose, currentConfig, onApply, isRunning }: ConfigDialogProps) {
  const [cpuType, setCpuType] = useState(currentConfig.cpuType);
  const [memoryKb, setMemoryKb] = useState(String(currentConfig.memoryKb));
  const [clockMhz, setClockMhz] = useState(String(currentConfig.clockMhz));
  const [videoCard, setVideoCard] = useState(currentConfig.videoCard);

  // Sync local state from currentConfig whenever the dialog opens
  useEffect(() => {
    if (opened) {
      setCpuType(currentConfig.cpuType);
      setMemoryKb(String(currentConfig.memoryKb));
      setClockMhz(String(currentConfig.clockMhz));
      setVideoCard(currentConfig.videoCard);
    }
  }, [opened, currentConfig]);

  const handleApply = () => {
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
    <Modal
      opened={opened}
      onClose={onClose}
      title="System Configuration"
      size="sm"
    >
      <Stack gap="md">
        {isRunning && (
          <Alert color="yellow" title="Warning">
            Applying configuration will reset the emulator and stop execution.
          </Alert>
        )}

        <div>
          <Text size="sm" fw={500} mb={4}>CPU Type</Text>
          <Select
            data={CPU_OPTIONS}
            value={cpuType}
            onChange={(v) => v && setCpuType(v)}
          />
        </div>

        <div>
          <Text size="sm" fw={500} mb={4}>Memory</Text>
          <Select
            data={MEMORY_OPTIONS}
            value={memoryKb}
            onChange={(v) => v && setMemoryKb(v)}
          />
          {needsExtendedRam && cpuType === '8086' && (
            <Text size="xs" c="red" mt={2}>Extended memory requires 286 or later CPU</Text>
          )}
        </div>

        <div>
          <Text size="sm" fw={500} mb={4}>Clock Speed</Text>
          <Select
            data={CLOCK_OPTIONS}
            value={clockMhz}
            onChange={(v) => v && setClockMhz(v)}
          />
        </div>

        <div>
          <Text size="sm" fw={500} mb={4}>Video Card</Text>
          <Select
            data={VIDEO_CARD_OPTIONS}
            value={videoCard}
            onChange={(v) => v && setVideoCard(v)}
          />
        </div>

        <Group justify="flex-end" gap="xs">
          <Button variant="default" onClick={onClose}>Cancel</Button>
          <Button onClick={handleApply}>Apply &amp; Reset</Button>
        </Group>
      </Stack>
    </Modal>
  );
}

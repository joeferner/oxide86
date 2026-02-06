import { Group, Button, Text } from '@mantine/core'
import styles from './ControlGroup.module.scss'

interface BootControlProps {
  onBootA: () => void;
  onBootC: () => void;
  onReset: () => void;
  bootDrive: number;
}

export function BootControl({ onBootA, onBootC, onReset, bootDrive }: BootControlProps) {
  return (
    <div className={styles.controlGroup}>
      <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>Boot Control:</Text>
      <Group gap="xs">
        <Button
          onClick={onBootA}
          color="blue"
          size="compact-sm"
          leftSection={bootDrive === 0x00 ? <i className="bi bi-check-circle-fill"></i> : null}
          variant={bootDrive === 0x00 ? 'filled' : 'light'}
        >
          Boot from A:
        </Button>
        <Button
          onClick={onBootC}
          color="blue"
          size="compact-sm"
          leftSection={bootDrive === 0x80 ? <i className="bi bi-check-circle-fill"></i> : null}
          variant={bootDrive === 0x80 ? 'filled' : 'light'}
        >
          Boot from C:
        </Button>
        <Button onClick={onReset} color="red" size="compact-sm">Reset</Button>
      </Group>
    </div>
  )
}

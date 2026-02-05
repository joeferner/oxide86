import { Group, Button, Text } from '@mantine/core'
import styles from './ControlGroup.module.scss'

interface BootControlProps {
  onBootA: () => void;
  onBootC: () => void;
  onReset: () => void;
}

export function BootControl({ onBootA, onBootC, onReset }: BootControlProps) {
  return (
    <div className={styles.controlGroup}>
      <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>Boot Control:</Text>
      <Group gap="xs">
        <Button onClick={onBootA} color="blue" size="compact-sm">Boot from A:</Button>
        <Button onClick={onBootC} color="blue" size="compact-sm">Boot from C:</Button>
        <Button onClick={onReset} color="red" size="compact-sm">Reset</Button>
      </Group>
    </div>
  )
}

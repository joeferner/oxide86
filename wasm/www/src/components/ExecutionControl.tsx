import { Group, Button, Text } from '@mantine/core'
import styles from './ControlGroup.module.scss'

interface ExecutionControlProps {
  isRunning: boolean;
  onStart: () => void;
  onStop: () => void;
  onStep: () => void;
}

export function ExecutionControl({ isRunning, onStart, onStop, onStep }: ExecutionControlProps) {
  return (
    <div className={styles.controlGroup}>
      <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>Execution Control:</Text>
      <Group gap="xs">
        <Button onClick={onStart} disabled={isRunning} color="green" size="compact-sm">Start</Button>
        <Button onClick={onStop} disabled={!isRunning} color="red" size="compact-sm">Stop</Button>
        <Button onClick={onStep} color="blue" size="compact-sm">Step</Button>
      </Group>
    </div>
  )
}

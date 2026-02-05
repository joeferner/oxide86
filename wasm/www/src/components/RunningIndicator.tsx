import { Group, Badge } from '@mantine/core'

interface RunningIndicatorProps {
  isRunning: boolean;
}

export function RunningIndicator({ isRunning }: RunningIndicatorProps) {
  return (
    <Group mt="sm" gap="xs">
      <div
        style={{
          width: 12,
          height: 12,
          borderRadius: '50%',
          backgroundColor: isRunning ? 'var(--mantine-color-green-6)' : 'var(--mantine-color-dark-4)',
          boxShadow: isRunning ? '0 0 8px var(--mantine-color-green-6)' : 'none',
          transition: 'all 0.3s ease'
        }}
      />
      <Badge color={isRunning ? 'green' : 'gray'} variant="light">
        {isRunning ? 'RUNNING' : 'STOPPED'}
      </Badge>
    </Group>
  )
}

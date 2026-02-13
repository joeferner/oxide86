import { Group, Text, Code } from '@mantine/core';

interface Performance {
    target: number;
    actual: number;
}

interface PerformanceDisplayProps {
    performance: Performance;
}

export function PerformanceDisplay({ performance }: PerformanceDisplayProps): React.ReactElement {
    return (
        <Group
            gap="md"
            justify="center"
            style={{
                padding: '8px 12px',
                backgroundColor: 'var(--mantine-color-dark-7)',
                borderRadius: '4px',
                fontFamily: 'monospace',
            }}
        >
            <Group gap="xs">
                <Text c="dimmed" fw={600} size="sm">
                    Target:
                </Text>
                <Code color="green">{performance.target.toFixed(2)} MHz</Code>
            </Group>
            <Group gap="xs">
                <Text c="dimmed" fw={600} size="sm">
                    Actual:
                </Text>
                <Code color="green">{performance.actual.toFixed(2)} MHz</Code>
            </Group>
        </Group>
    );
}

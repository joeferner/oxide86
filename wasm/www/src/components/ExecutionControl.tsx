import { Group, Button, Text } from '@mantine/core';
import styles from './ControlGroup.module.scss';

interface ExecutionControlProps {
    mode: 'boot' | 'program';
    isRunning: boolean;
    hasBooted: boolean;
    onAction: () => void;
    onReset: () => void;
}

export function ExecutionControl({
    mode,
    isRunning,
    hasBooted,
    onAction,
    onReset,
}: ExecutionControlProps): React.ReactElement {
    // Determine button label, color, and icon based on state
    let actionLabel: string;
    let actionColor: string;
    let actionIcon: React.ReactNode;

    if (!hasBooted) {
        actionLabel = mode === 'boot' ? 'Boot' : 'Start';
        actionColor = 'green';
        actionIcon = <i className="bi bi-play-fill"></i>;
    } else if (isRunning) {
        actionLabel = 'Pause';
        actionColor = 'yellow';
        actionIcon = <i className="bi bi-pause-fill"></i>;
    } else {
        actionLabel = 'Resume';
        actionColor = 'green';
        actionIcon = <i className="bi bi-play-fill"></i>;
    }

    return (
        <div className={styles.controlGroup}>
            <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>
                Execution Control:
            </Text>
            <Group gap="xs">
                <Button onClick={onAction} color={actionColor} size="compact-sm" leftSection={actionIcon}>
                    {actionLabel}
                </Button>
                <Button
                    onClick={onReset}
                    color="red"
                    size="compact-sm"
                    leftSection={<i className="bi bi-arrow-clockwise"></i>}
                >
                    Reset
                </Button>
            </Group>
        </div>
    );
}

import { Code } from '@mantine/core';
import React from 'react';

interface StatusDisplayProps {
    status: string;
}

export function StatusDisplay({ status }: StatusDisplayProps): React.ReactElement {
    const timestamp = new Date().toLocaleTimeString();
    return (
        <Code
            block
            mt="md"
            style={{
                backgroundColor: 'var(--mantine-color-dark-7)',
                borderLeft: '4px solid var(--mantine-color-blue-6)',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
                overflowWrap: 'break-word',
            }}
        >
            [{timestamp}] {status}
        </Code>
    );
}

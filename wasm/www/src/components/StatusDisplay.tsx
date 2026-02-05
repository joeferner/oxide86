import { Code } from '@mantine/core'

interface StatusDisplayProps {
  status: string;
}

export function StatusDisplay({ status }: StatusDisplayProps) {
  const timestamp = new Date().toLocaleTimeString()
  return (
    <Code
      block
      mt="md"
      style={{
        backgroundColor: 'var(--mantine-color-dark-7)',
        borderLeft: '4px solid var(--mantine-color-blue-6)',
        whiteSpace: 'pre-wrap',
        wordBreak: 'break-word',
        overflowWrap: 'break-word'
      }}
    >
      [{timestamp}] {status}
    </Code>
  )
}

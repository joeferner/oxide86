interface StatusDisplayProps {
  status: string;
}

export function StatusDisplay({ status }: StatusDisplayProps) {
  const timestamp = new Date().toLocaleTimeString()
  return (
    <div id="status">
      [{timestamp}] {status}
    </div>
  )
}

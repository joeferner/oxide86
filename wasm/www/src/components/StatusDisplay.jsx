export function StatusDisplay({ status }) {
  const timestamp = new Date().toLocaleTimeString()
  return (
    <div id="status">
      [{timestamp}] {status}
    </div>
  )
}

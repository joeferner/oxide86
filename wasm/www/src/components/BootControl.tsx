interface BootControlProps {
  onBootA: () => void;
  onBootC: () => void;
  onReset: () => void;
}

export function BootControl({ onBootA, onBootC, onReset }: BootControlProps) {
  return (
    <div className="control-group">
      <label className="control-label">Boot Control:</label>
      <button onClick={onBootA} className="btn-secondary">Boot from A:</button>
      <button onClick={onBootC} className="btn-secondary">Boot from C:</button>
      <button onClick={onReset} className="btn-danger">Reset</button>
    </div>
  )
}

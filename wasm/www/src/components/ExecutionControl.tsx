interface ExecutionControlProps {
  isRunning: boolean;
  onStart: () => void;
  onStop: () => void;
  onStep: () => void;
}

export function ExecutionControl({ isRunning, onStart, onStop, onStep }: ExecutionControlProps) {
  return (
    <div className="control-group">
      <label className="control-label">Execution Control:</label>
      <button onClick={onStart} disabled={isRunning}>Start</button>
      <button onClick={onStop} disabled={!isRunning} className="btn-danger">Stop</button>
      <button onClick={onStep} className="btn-secondary">Step</button>
    </div>
  )
}

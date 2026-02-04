interface RunningIndicatorProps {
  isRunning: boolean;
}

export function RunningIndicator({ isRunning }: RunningIndicatorProps) {
  return (
    <div id="running-indicator">
      <div className={`indicator-led ${isRunning ? 'running' : ''}`}></div>
      <div className={`indicator-text ${isRunning ? 'running' : ''}`}>
        {isRunning ? 'RUNNING' : 'STOPPED'}
      </div>
    </div>
  )
}

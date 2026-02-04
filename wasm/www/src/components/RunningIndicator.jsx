export function RunningIndicator({ isRunning }) {
  return (
    <div id="running-indicator">
      <div className={`indicator-led ${isRunning ? 'running' : ''}`}></div>
      <div className={`indicator-text ${isRunning ? 'running' : ''}`}>
        {isRunning ? 'RUNNING' : 'STOPPED'}
      </div>
    </div>
  )
}

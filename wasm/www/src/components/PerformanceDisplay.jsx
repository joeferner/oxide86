export function PerformanceDisplay({ performance }) {
  return (
    <div id="performance-display" className="performance">
      <div className="perf-label">Target:</div>
      <div className="perf-value">{performance.target.toFixed(2)} MHz</div>
      <div className="perf-label">Actual:</div>
      <div className="perf-value">{performance.actual.toFixed(2)} MHz</div>
    </div>
  )
}

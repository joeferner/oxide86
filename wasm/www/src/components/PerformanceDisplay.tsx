interface Performance {
  target: number;
  actual: number;
}

interface PerformanceDisplayProps {
  performance: Performance;
}

export function PerformanceDisplay({ performance }: PerformanceDisplayProps) {
  return (
    <div id="performance-display" className="performance">
      <div className="perf-label">Target:</div>
      <div className="perf-value">{performance.target.toFixed(2)} MHz</div>
      <div className="perf-label">Actual:</div>
      <div className="perf-value">{performance.actual.toFixed(2)} MHz</div>
    </div>
  )
}

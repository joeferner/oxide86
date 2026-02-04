interface InfoBoxProps {
  isPointerLocked: boolean;
}

export function InfoBox({ isPointerLocked }: InfoBoxProps) {
  if (isPointerLocked) {
    return (
      <div className="info-box locked">
        <strong>Mouse Locked:</strong> Mouse is locked to canvas for infinite movement.{' '}
        <strong>Press F12 to exit mouse lock.</strong>
      </div>
    )
  }

  return (
    <div className="info-box">
      <strong>Instructions:</strong> Load a disk image, click Boot, then Start to run the emulator.
      Use keyboard and mouse in the canvas area. Click on the canvas to lock the mouse for infinite movement.
    </div>
  )
}

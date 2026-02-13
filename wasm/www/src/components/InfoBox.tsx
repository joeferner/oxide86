import { Alert } from '@mantine/core';

interface InfoBoxProps {
    isPointerLocked: boolean;
}

export function InfoBox({ isPointerLocked }: InfoBoxProps): React.ReactElement {
    if (isPointerLocked) {
        return (
            <Alert color="violet" title="Mouse Locked">
                Mouse is locked to canvas for infinite movement. <strong>Press F12 to exit mouse lock.</strong>
            </Alert>
        );
    }

    return (
        <Alert color="yellow" title="Instructions">
            Load a disk image, click Boot, then Start to run the emulator. Use keyboard and mouse in the canvas area.
            Click on the canvas to lock the mouse for infinite movement.
        </Alert>
    );
}

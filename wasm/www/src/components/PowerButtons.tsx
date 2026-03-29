import React from 'react';
import { ActionIcon, Tooltip } from '@mantine/core';
import { state } from '../state';

export function PowerButtons(): React.ReactElement {
    const running = state.computer.value !== null;

    return (
        <>
            <Tooltip label="Power On" position="right">
                <ActionIcon
                    size="lg"
                    variant="subtle"
                    color="green"
                    disabled={running}
                    onClick={() => {
                        void state.powerOn();
                    }}
                    aria-label="Power On"
                >
                    <i className="bi bi-power" />
                </ActionIcon>
            </Tooltip>
            <Tooltip label="Reboot" position="right">
                <ActionIcon
                    size="lg"
                    variant="subtle"
                    disabled={!running}
                    onClick={() => {
                        state.reboot();
                    }}
                    aria-label="Reboot"
                >
                    <i className="bi bi-arrow-clockwise" />
                </ActionIcon>
            </Tooltip>
        </>
    );
}

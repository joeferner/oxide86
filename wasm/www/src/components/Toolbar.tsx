import React, { useState } from 'react';
import { ActionIcon, Tooltip } from '@mantine/core';
import { DriveButton } from './DriveButton';
import { DrivePanel } from './DrivePanel';
import { PowerPanel } from './PowerPanel';
import { MachineConfig } from './MachineConfig';
import styles from './Toolbar.module.scss';

type DriveId = 0 | 1 | 'hdd';

const driveConfigs = [
    { label: 'A:', drive: 0 as DriveId, icon: 'bi-floppy', canEject: true },
    { label: 'B:', drive: 1 as DriveId, icon: 'bi-floppy', canEject: true },
    { label: 'C:', drive: 'hdd' as DriveId, icon: 'bi-hdd', canEject: false },
];

type Panel = DriveId | 'config' | 'power';

export function Toolbar(): React.ReactElement {
    const [activePanel, setActivePanel] = useState<Panel | null>(null);

    const handleDriveSelect = (drive: DriveId): void => {
        setActivePanel((prev) => (prev === drive ? null : drive));
    };

    const handleConfigToggle = (): void => {
        setActivePanel((prev) => (prev === 'config' ? null : 'config'));
    };

    const handlePowerToggle = (): void => {
        setActivePanel((prev) => (prev === 'power' ? null : 'power'));
    };

    const selectedDriveConfig = driveConfigs.find((d) => d.drive === activePanel);

    return (
        <div className={styles.toolbarContainer}>
            <div className={styles.toolbar}>
                <div className={styles.group}>
                    {driveConfigs.map((cfg) => (
                        <DriveButton
                            key={String(cfg.drive)}
                            label={cfg.label}
                            drive={cfg.drive}
                            icon={cfg.icon}
                            selected={activePanel === cfg.drive}
                            onSelect={() => {
                                handleDriveSelect(cfg.drive);
                            }}
                        />
                    ))}
                </div>
                <div className={styles.group}>
                    <Tooltip label="Power" position="left">
                        <ActionIcon
                            variant={activePanel === 'power' ? 'filled' : 'subtle'}
                            size="lg"
                            aria-label="Power"
                            onClick={handlePowerToggle}
                        >
                            <i className="bi bi-power" />
                        </ActionIcon>
                    </Tooltip>
                    <Tooltip label="Machine settings" position="left">
                        <ActionIcon
                            variant={activePanel === 'config' ? 'filled' : 'subtle'}
                            size="lg"
                            aria-label="Machine settings"
                            onClick={handleConfigToggle}
                        >
                            <i className="bi bi-gear" />
                        </ActionIcon>
                    </Tooltip>
                </div>
            </div>
            {selectedDriveConfig && (
                <DrivePanel
                    drive={selectedDriveConfig.drive}
                    label={selectedDriveConfig.label}
                    canEject={selectedDriveConfig.canEject}
                />
            )}
            {activePanel === 'power' && <PowerPanel />}
            {activePanel === 'config' && <MachineConfig />}
        </div>
    );
}

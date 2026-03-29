import React, { useLayoutEffect, useRef, useState } from 'react';
import { ActionIcon, Tooltip } from '@mantine/core';
import { useSignalEffect } from '@preact/signals-react';
import { DriveButton } from './DriveButton';
import { DrivePanel } from './DrivePanel';
import { PowerPanel } from './PowerPanel';
import { MachineConfig } from './MachineConfig';
import { state } from '../state';
import styles from './Toolbar.module.scss';

type DriveId = 0 | 1 | 'hdd';

const driveConfigs = [
    { label: 'A:', drive: 0 as DriveId, icon: 'bi-floppy', canEject: true },
    { label: 'B:', drive: 1 as DriveId, icon: 'bi-floppy', canEject: true },
    { label: 'C:', drive: 'hdd' as DriveId, icon: 'bi-hdd', canEject: false },
];

type Panel = DriveId | 'config' | 'power-confirm' | 'reboot-confirm';

export function Toolbar(): React.ReactElement {
    const [activePanel, setActivePanel] = useState<Panel | null>(null);
    const [isRunning, setIsRunning] = useState(() => state.computer.peek() !== null);
    const [powerState, setPowerState] = useState(state.powerState.peek());
    const containerRef = useRef<HTMLDivElement>(null);
    const buttonRefs = useRef<Map<string, HTMLDivElement>>(new Map());
    const panelWrapperRef = useRef<HTMLDivElement>(null);

    useSignalEffect(() => {
        setIsRunning(state.computer.value !== null);
    });

    useSignalEffect(() => {
        setPowerState(state.powerState.value);
    });

    useLayoutEffect(() => {
        const wrapper = panelWrapperRef.current;
        if (!wrapper || activePanel === null || !containerRef.current) {
            return;
        }
        const panelToButtonKey: Record<string, string> = {
            'power-confirm': 'power',
            'reboot-confirm': 'reboot',
        };
        const btnKey = panelToButtonKey[String(activePanel)] ?? String(activePanel);
        const btn = buttonRefs.current.get(btnKey);
        if (!btn) {
            return;
        }
        const containerRect = containerRef.current.getBoundingClientRect();
        const btnRect = btn.getBoundingClientRect();
        wrapper.style.setProperty('--chevron-top', `${btnRect.top - containerRect.top + btnRect.height / 2}px`);
    }, [activePanel]);

    const setButtonRef = (key: string) => (el: HTMLDivElement | null) => {
        if (el) {
            buttonRefs.current.set(key, el);
        } else {
            buttonRefs.current.delete(key);
        }
    };

    const handleDriveSelect = (drive: DriveId): void => {
        setActivePanel((prev) => (prev === drive ? null : drive));
    };

    const handleConfigToggle = (): void => {
        setActivePanel((prev) => (prev === 'config' ? null : 'config'));
    };

    const handlePowerClick = (): void => {
        if (isRunning) {
            setActivePanel((prev) => (prev === 'power-confirm' ? null : 'power-confirm'));
        } else {
            void state.powerOn();
        }
    };

    const handleRebootClick = (): void => {
        setActivePanel((prev) => (prev === 'reboot-confirm' ? null : 'reboot-confirm'));
    };

    const selectedDriveConfig = driveConfigs.find((d) => d.drive === activePanel);
    const hasPanel =
        selectedDriveConfig != null ||
        activePanel === 'power-confirm' ||
        activePanel === 'reboot-confirm' ||
        activePanel === 'config';

    return (
        <div className={styles.toolbarContainer} ref={containerRef}>
            <div className={styles.toolbar}>
                <div className={styles.group}>
                    {driveConfigs.map((cfg) => (
                        <div key={String(cfg.drive)} ref={setButtonRef(String(cfg.drive))}>
                            <DriveButton
                                label={cfg.label}
                                drive={cfg.drive}
                                icon={cfg.icon}
                                selected={activePanel === cfg.drive}
                                onSelect={() => {
                                    handleDriveSelect(cfg.drive);
                                }}
                            />
                        </div>
                    ))}
                </div>
                <div className={styles.group}>
                    <div ref={setButtonRef('reboot')}>
                        <Tooltip label="Reboot" position="left">
                            <ActionIcon
                                variant={activePanel === 'reboot-confirm' ? 'filled' : 'subtle'}
                                size="lg"
                                aria-label="Reboot"
                                disabled={!isRunning}
                                onClick={handleRebootClick}
                            >
                                <i className="bi bi-arrow-clockwise" />
                            </ActionIcon>
                        </Tooltip>
                    </div>
                    <div ref={setButtonRef('power')} className={styles.powerButtonWrapper}>
                        <Tooltip label={isRunning ? 'Power off' : 'Power on'} position="left">
                            <ActionIcon
                                variant={activePanel === 'power-confirm' ? 'filled' : 'subtle'}
                                size="lg"
                                aria-label="Power"
                                onClick={handlePowerClick}
                            >
                                <i className="bi bi-power" />
                            </ActionIcon>
                        </Tooltip>
                        <span className={`${styles.powerDot} ${styles[`powerDot_${powerState}`]}`} />
                    </div>
                    <div ref={setButtonRef('config')}>
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
            </div>
            {hasPanel && (
                <div className={styles.panelWrapper} ref={panelWrapperRef}>
                    {selectedDriveConfig && (
                        <DrivePanel
                            key={String(selectedDriveConfig.drive)}
                            drive={selectedDriveConfig.drive}
                            label={selectedDriveConfig.label}
                            canEject={selectedDriveConfig.canEject}
                        />
                    )}
                    {activePanel === 'power-confirm' && (
                        <PowerPanel mode="power" onClose={() => { setActivePanel(null); }} />
                    )}
                    {activePanel === 'reboot-confirm' && (
                        <PowerPanel mode="reboot" onClose={() => { setActivePanel(null); }} />
                    )}
                    {activePanel === 'config' && <MachineConfig />}
                </div>
            )}
        </div>
    );
}

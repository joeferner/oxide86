import React, { useState } from 'react';
import { ActionIcon, Tooltip } from '@mantine/core';
import { DriveButton } from './DriveButton';
import { PowerButtons } from './PowerButtons';
import { MachineConfig } from './MachineConfig';
import styles from './Toolbar.module.scss';

export function Toolbar(): React.ReactElement {
    const [configOpen, setConfigOpen] = useState(false);

    return (
        <>
            <div className={styles.toolbar}>
                <div className={styles.group}>
                    <DriveButton label="A:" drive={0} icon="bi-floppy" canEject />
                    <DriveButton label="B:" drive={1} icon="bi-floppy" canEject />
                    <DriveButton label="C:" drive="hdd" icon="bi-hdd" canEject={false} />
                </div>
                <div className={styles.group}>
                    <PowerButtons />
                </div>
                <div className={styles.group}>
                    <Tooltip label="Machine settings" position="left">
                        <ActionIcon
                            variant="subtle"
                            size="lg"
                            aria-label="Machine settings"
                            onClick={() => {
                                setConfigOpen(true);
                            }}
                        >
                            <i className="bi bi-gear" />
                        </ActionIcon>
                    </Tooltip>
                </div>
            </div>
            <MachineConfig
                opened={configOpen}
                onClose={() => {
                    setConfigOpen(false);
                }}
            />
        </>
    );
}

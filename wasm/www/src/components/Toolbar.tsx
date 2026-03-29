import React from 'react';
import { DriveButton } from './DriveButton';
import { PowerButtons } from './PowerButtons';
import styles from './Toolbar.module.scss';

export function Toolbar(): React.ReactElement {
    return (
        <div className={styles.toolbar}>
            <div className={styles.group}>
                <DriveButton label="A:" drive={0} icon="bi-floppy" canEject />
                <DriveButton label="B:" drive={1} icon="bi-floppy" canEject />
                <DriveButton label="C:" drive="hdd" icon="bi-hdd" canEject={false} />
            </div>
            <div className={styles.group}>
                <PowerButtons />
            </div>
        </div>
    );
}

import React from 'react';
import { Screen } from './components/Screen';
import { StatusBar } from './components/StatusBar';
import { Toolbar } from './components/Toolbar';
import styles from './App.module.scss';

export function App(): React.ReactElement {
    return (
        <div className={styles.page}>
            <div className={styles.center}>
                <div className={styles.screenStack}>
                    <Screen />
                    <StatusBar />
                </div>
                <Toolbar />
            </div>
        </div>
    );
}

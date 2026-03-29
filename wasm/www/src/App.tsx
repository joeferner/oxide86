import React from 'react';
import { Screen } from './components/Screen';
import { StatusBar } from './components/StatusBar';
import { Toolbar } from './components/Toolbar';
import styles from './App.module.scss';

export function App(): React.ReactElement {
    return (
        <div className={styles.page}>
            <div className={styles.content}>
                <div className={styles.header}>
                    <img src="/logo.png" alt="Oxide86" className={styles.logo} />
                    <div>
                        <h1 className={styles.title}>Oxide86</h1>
                        <p className={styles.subtitle}>x86 Rust Emulator</p>
                    </div>
                </div>
                <div className={styles.center}>
                    <div className={styles.screenStack}>
                        <Screen />
                        <StatusBar />
                    </div>
                    <Toolbar />
                </div>
            </div>
        </div>
    );
}

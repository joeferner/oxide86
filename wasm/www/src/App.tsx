import React from 'react';
import { Header } from './components/Header';
import { Screen } from './components/Screen';
import { Toolbar } from './components/Toolbar';
import styles from './App.module.scss';

export function App(): React.ReactElement {
    return (
        <div className={styles.page}>
            <div className={styles.content}>
                <Header />
                <div className={styles.center}>
                    <Screen />
                    <Toolbar />
                </div>
            </div>
        </div>
    );
}

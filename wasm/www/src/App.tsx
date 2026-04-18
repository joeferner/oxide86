import React, { useEffect, useRef } from 'react';
import { Header } from './components/Header';
import { Screen } from './components/Screen';
import { Toolbar } from './components/Toolbar';
import styles from './App.module.scss';

export function App(): React.ReactElement {
    const centerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const center = centerRef.current;
        if (!center) return;

        const canvas = center.querySelector('canvas');
        if (!canvas) return;

        const updateHeight = () => {
            center.style.height = `${canvas.offsetHeight}px`;
        };

        updateHeight();
        const ro = new ResizeObserver(updateHeight);
        ro.observe(canvas);
        return () => ro.disconnect();
    }, []);

    return (
        <div className={styles.page}>
            <div className={styles.content}>
                <Header />
                <div className={styles.center} ref={centerRef}>
                    <Screen />
                    <Toolbar />
                </div>
            </div>
        </div>
    );
}

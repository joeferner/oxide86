import ReactDOM from 'react-dom/client';
import { MantineProvider, createTheme } from '@mantine/core';
import { Notifications } from '@mantine/notifications';
import '@mantine/core/styles.css';
import '@mantine/notifications/styles.css';
import 'bootstrap-icons/font/bootstrap-icons.css';
import initWasm from 'oxide86-wasm';
import { App } from './App';
import './styles/global.scss';

const theme = createTheme({
    primaryColor: 'blue',
    colors: {
        dark: [
            '#e0e0e0',
            '#b0b0b0',
            '#888888',
            '#666666',
            '#444444',
            '#3a3a3a',
            '#2a2a2a',
            '#1a1a1a',
            '#0a0a0a',
            '#000000',
        ],
    },
    fontFamily: 'Arial, sans-serif',
    headings: { fontFamily: 'Arial, sans-serif' },
});

const root = document.getElementById('root');
if (!root) {
    throw new Error('cannot find root element');
}

void initWasm().then(() => {
    ReactDOM.createRoot(root).render(
        <MantineProvider theme={theme} defaultColorScheme="dark">
            <Notifications position="top-right" autoClose={4000} />
            <App />
        </MantineProvider>
    );
});

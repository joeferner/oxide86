import ReactDOM from 'react-dom/client'
import { MantineProvider, createTheme } from '@mantine/core'
import '@mantine/core/styles.css'
import App from './App'
import './styles/App.css'

const theme = createTheme({
  primaryColor: 'green',
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
})

ReactDOM.createRoot(document.getElementById('root')!).render(
  <MantineProvider theme={theme} defaultColorScheme="dark">
    <App />
  </MantineProvider>
)

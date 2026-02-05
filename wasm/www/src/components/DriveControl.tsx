import { useState } from 'react'
import { Group, Button, FileButton, Text } from '@mantine/core'
import { Emu86Computer } from '../../pkg/emu86_wasm'
import styles from './ControlGroup.module.scss'

interface DriveControlProps {
  computer: Emu86Computer | null;
  onStatusUpdate: (message: string) => void;
}

async function loadFile(file: File): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = (e) => resolve(new Uint8Array(e.target?.result as ArrayBuffer))
    reader.onerror = reject
    reader.readAsArrayBuffer(file)
  })
}

export function DriveControl({ computer, onStatusUpdate }: DriveControlProps) {
  const [floppyAFile, setFloppyAFile] = useState<File | null>(null)
  const [floppyBFile, setFloppyBFile] = useState<File | null>(null)
  const [hddFile, setHddFile] = useState<File | null>(null)

  const handleLoadFloppyA = async () => {
    if (!floppyAFile) {
      onStatusUpdate('Please select a file first')
      return
    }

    try {
      onStatusUpdate('Loading floppy A...')
      const data = await loadFile(floppyAFile)
      computer?.load_floppy(0, data)
      onStatusUpdate(`Loaded floppy A: ${floppyAFile.name} (${data.length} bytes)`)
    } catch (e) {
      onStatusUpdate(`Error loading floppy A: ${e}`)
      console.error(e)
    }
  }

  const handleEjectFloppyA = () => {
    try {
      computer?.eject_floppy(0)
      setFloppyAFile(null)
      onStatusUpdate('Floppy A ejected')
    } catch (e) {
      onStatusUpdate(`Error ejecting floppy A: ${e}`)
      console.error(e)
    }
  }

  const handleLoadFloppyB = async () => {
    if (!floppyBFile) {
      onStatusUpdate('Please select a file first')
      return
    }

    try {
      onStatusUpdate('Loading floppy B...')
      const data = await loadFile(floppyBFile)
      computer?.load_floppy(1, data)
      onStatusUpdate(`Loaded floppy B: ${floppyBFile.name} (${data.length} bytes)`)
    } catch (e) {
      onStatusUpdate(`Error loading floppy B: ${e}`)
      console.error(e)
    }
  }

  const handleEjectFloppyB = () => {
    try {
      computer?.eject_floppy(1)
      setFloppyBFile(null)
      onStatusUpdate('Floppy B ejected')
    } catch (e) {
      onStatusUpdate(`Error ejecting floppy B: ${e}`)
      console.error(e)
    }
  }

  const handleLoadHDD = async () => {
    if (!hddFile) {
      onStatusUpdate('Please select a file first')
      return
    }

    try {
      onStatusUpdate('Loading hard drive C...')
      const data = await loadFile(hddFile)
      computer?.add_hard_drive(data)
      onStatusUpdate(`Loaded hard drive C: ${hddFile.name} (${data.length} bytes)`)
    } catch (e) {
      onStatusUpdate(`Error loading hard drive: ${e}`)
      console.error(e)
    }
  }

  if (!computer) return null

  return (
    <>
      <div className={styles.controlGroup}>
        <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>Floppy Drive A:</Text>
        <Group gap="xs">
          <FileButton onChange={setFloppyAFile} accept=".img,.ima,.dsk">
            {(props) => <Button {...props} size="compact-sm" variant="default">{floppyAFile ? floppyAFile.name : 'Choose File'}</Button>}
          </FileButton>
          <Button onClick={handleLoadFloppyA} size="compact-sm" color="green" disabled={!floppyAFile}>Load A:</Button>
          <Button onClick={handleEjectFloppyA} size="compact-sm" color="red">Eject A:</Button>
        </Group>
      </div>

      <div className={styles.controlGroup}>
        <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>Floppy Drive B:</Text>
        <Group gap="xs">
          <FileButton onChange={setFloppyBFile} accept=".img,.ima,.dsk">
            {(props) => <Button {...props} size="compact-sm" variant="default">{floppyBFile ? floppyBFile.name : 'Choose File'}</Button>}
          </FileButton>
          <Button onClick={handleLoadFloppyB} size="compact-sm" color="green" disabled={!floppyBFile}>Load B:</Button>
          <Button onClick={handleEjectFloppyB} size="compact-sm" color="red">Eject B:</Button>
        </Group>
      </div>

      <div className={styles.controlGroup}>
        <Text fw={700} c="dimmed" style={{ minWidth: 150, textAlign: 'right' }}>Hard Drive C:</Text>
        <Group gap="xs">
          <FileButton onChange={setHddFile} accept=".img,.ima,.dsk,.vhd">
            {(props) => <Button {...props} size="compact-sm" variant="default">{hddFile ? hddFile.name : 'Choose File'}</Button>}
          </FileButton>
          <Button onClick={handleLoadHDD} size="compact-sm" color="green" disabled={!hddFile}>Load C:</Button>
        </Group>
      </div>
    </>
  )
}

import { useRef } from 'react'

async function loadFile(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = (e) => resolve(new Uint8Array(e.target.result))
    reader.onerror = reject
    reader.readAsArrayBuffer(file)
  })
}

export function DriveControl({ computer, onStatusUpdate }) {
  const floppyAInputRef = useRef(null)
  const floppyBInputRef = useRef(null)
  const hddInputRef = useRef(null)

  const handleLoadFloppyA = async () => {
    const input = floppyAInputRef.current
    if (!input || input.files.length === 0) {
      onStatusUpdate('Please select a file first')
      return
    }

    try {
      onStatusUpdate('Loading floppy A...')
      const data = await loadFile(input.files[0])
      computer.load_floppy(0, data)
      onStatusUpdate(`Loaded floppy A: ${input.files[0].name} (${data.length} bytes)`)
    } catch (e) {
      onStatusUpdate(`Error loading floppy A: ${e}`)
      console.error(e)
    }
  }

  const handleEjectFloppyA = () => {
    try {
      computer.eject_floppy(0)
      onStatusUpdate('Floppy A ejected')
    } catch (e) {
      onStatusUpdate(`Error ejecting floppy A: ${e}`)
      console.error(e)
    }
  }

  const handleLoadFloppyB = async () => {
    const input = floppyBInputRef.current
    if (!input || input.files.length === 0) {
      onStatusUpdate('Please select a file first')
      return
    }

    try {
      onStatusUpdate('Loading floppy B...')
      const data = await loadFile(input.files[0])
      computer.load_floppy(1, data)
      onStatusUpdate(`Loaded floppy B: ${input.files[0].name} (${data.length} bytes)`)
    } catch (e) {
      onStatusUpdate(`Error loading floppy B: ${e}`)
      console.error(e)
    }
  }

  const handleEjectFloppyB = () => {
    try {
      computer.eject_floppy(1)
      onStatusUpdate('Floppy B ejected')
    } catch (e) {
      onStatusUpdate(`Error ejecting floppy B: ${e}`)
      console.error(e)
    }
  }

  const handleLoadHDD = async () => {
    const input = hddInputRef.current
    if (!input || input.files.length === 0) {
      onStatusUpdate('Please select a file first')
      return
    }

    try {
      onStatusUpdate('Loading hard drive C...')
      const data = await loadFile(input.files[0])
      computer.add_hard_drive(data)
      onStatusUpdate(`Loaded hard drive C: ${input.files[0].name} (${data.length} bytes)`)
    } catch (e) {
      onStatusUpdate(`Error loading hard drive: ${e}`)
      console.error(e)
    }
  }

  if (!computer) return null

  return (
    <>
      <div className="control-group">
        <label className="control-label">Floppy Drive A:</label>
        <input
          ref={floppyAInputRef}
          type="file"
          className="file-input"
          accept=".img,.ima,.dsk"
        />
        <button onClick={handleLoadFloppyA}>Load A:</button>
        <button onClick={handleEjectFloppyA} className="btn-danger">Eject A:</button>
      </div>

      <div className="control-group">
        <label className="control-label">Floppy Drive B:</label>
        <input
          ref={floppyBInputRef}
          type="file"
          className="file-input"
          accept=".img,.ima,.dsk"
        />
        <button onClick={handleLoadFloppyB}>Load B:</button>
        <button onClick={handleEjectFloppyB} className="btn-danger">Eject B:</button>
      </div>

      <div className="control-group">
        <label className="control-label">Hard Drive C:</label>
        <input
          ref={hddInputRef}
          type="file"
          className="file-input"
          accept=".img,.ima,.dsk,.vhd"
        />
        <button onClick={handleLoadHDD}>Load C:</button>
      </div>
    </>
  )
}

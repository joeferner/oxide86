# Plan: Server Image Library (images.json)

## Overview

Add a server-side image library to the WASM interface. A file called `images.json` is fetched
from the server at startup and lists available disk images (floppy, hard drive, CD-ROM) with
names and descriptions. A picker UI appears next to the existing "Choose File" buttons in
`DriveControl.tsx` whenever images of that type are available, letting users load images
directly from the server without selecting a local file.

---

## File: `wasm/www/public/images.json`

Create this file in `public/` so Vite serves it at `/images.json`.

```json
{
  "floppy": [],
  "hdd": [],
  "cdrom": []
}
```

**Schema** — each entry in any list:
```json
{
  "name": "Human-readable label shown in picker",
  "description": "Short description shown as subtitle",
  "url": "relative/path/to/image.img"
}
```

URLs are resolved relative to the page origin (`new URL(entry.url, window.location.href)`).
Real images should be placed in `public/images/floppy/`, `public/images/hdd/`, etc. and
referenced accordingly.

---

## File: `wasm/www/src/imageLibrary.ts`

Responsible for fetching, parsing and caching `images.json` once.

```typescript
export interface DiskImage {
    name: string;
    description: string;
    url: string;
}

export interface ImagesLibrary {
    floppy: DiskImage[];
    hdd: DiskImage[];
    cdrom: DiskImage[];
}

// Module-level cache (fetched once on first call)
let cache: ImagesLibrary | null = null;
let fetchPromise: Promise<ImagesLibrary> | null = null;

export async function fetchImagesLibrary(): Promise<ImagesLibrary> {
    if (cache) return cache;
    if (!fetchPromise) {
        fetchPromise = fetch('/images.json')
            .then((r) => r.json())
            .then((data) => {
                cache = {
                    floppy: Array.isArray(data.floppy) ? data.floppy : [],
                    hdd:    Array.isArray(data.hdd)    ? data.hdd    : [],
                    cdrom:  Array.isArray(data.cdrom)  ? data.cdrom  : [],
                };
                return cache!;
            })
            .catch(() => {
                // If the file is missing or malformed, silently return empty library
                cache = { floppy: [], hdd: [], cdrom: [] };
                return cache;
            });
    }
    return fetchPromise;
}

// Fetches a binary image from a URL entry (resolves relative URLs against page origin)
export async function fetchImageData(url: string): Promise<Uint8Array> {
    const resolved = new URL(url, window.location.href).href;
    const response = await fetch(resolved);
    if (!response.ok) throw new Error(`HTTP ${response.status} fetching ${resolved}`);
    const buffer = await response.arrayBuffer();
    return new Uint8Array(buffer);
}
```

---

## File: `wasm/www/src/components/ImageLibraryPicker.tsx`

New reusable component. Renders a small icon button that opens a Mantine `Menu` listing
available images of the requested type. Calls `onLoad(data, name)` when an image is selected.

```tsx
import { useState, useEffect } from 'react';
import { ActionIcon, Menu, Text, Tooltip } from '@mantine/core';
import { fetchImagesLibrary, fetchImageData, DiskImage } from '../imageLibrary';
import { status } from '../emulatorState';

type DriveType = 'floppy' | 'hdd' | 'cdrom';

interface ImageLibraryPickerProps {
    driveType: DriveType;
    onLoad: (data: Uint8Array, name: string) => void;
}

export function ImageLibraryPicker({ driveType, onLoad }: ImageLibraryPickerProps) {
    const [images, setImages] = useState<DiskImage[]>([]);
    const [loading, setLoading] = useState(false);

    useEffect(() => {
        fetchImagesLibrary().then((lib) => setImages(lib[driveType]));
    }, [driveType]);

    if (images.length === 0) return null;

    const handleSelect = async (image: DiskImage) => {
        try {
            setLoading(true);
            status.value = `Fetching ${image.name}...`;
            const data = await fetchImageData(image.url);
            onLoad(data, image.name);
        } catch (e) {
            status.value = `Error fetching ${image.name}: ${e}`;
            console.error(e);
        } finally {
            setLoading(false);
        }
    };

    return (
        <Menu shadow="md" width={260}>
            <Menu.Target>
                <Tooltip label="Select from server library">
                    <ActionIcon size="md" variant="default" loading={loading}>
                        <i className="bi bi-server"></i>
                    </ActionIcon>
                </Tooltip>
            </Menu.Target>
            <Menu.Dropdown>
                <Menu.Label>Server Images</Menu.Label>
                {images.map((img) => (
                    <Menu.Item
                        key={img.url}
                        onClick={() => { void handleSelect(img); }}
                    >
                        <Text size="sm" fw={500}>{img.name}</Text>
                        {img.description && (
                            <Text size="xs" c="dimmed">{img.description}</Text>
                        )}
                    </Menu.Item>
                ))}
            </Menu.Dropdown>
        </Menu>
    );
}
```

---

## Changes to `wasm/www/src/components/DriveControl.tsx`

Import `ImageLibraryPicker` and add it to the `<Group>` for each drive row, after the
"Choose File" button and before the eject/manage/download icons.

**Floppy A:** Add `ImageLibraryPicker` with `driveType="floppy"` and an `onLoad` that calls
`handleFloppyAFromServer(data, name)`:

```typescript
const handleFloppyAFromServer = async (data: Uint8Array, name: string): Promise<void> => {
    try {
        // Create a synthetic File-like entry so remount logic still works
        floppyAFile.value = new File([data], name);
        computer.value?.load_floppy(0, data);
        status.value = `Loaded floppy A: ${name} (${data.length} bytes)`;
    } catch (e) {
        status.value = `Error loading floppy A: ${e}`;
        console.error(e);
        floppyAFile.value = null;
    }
};
```

Wrap the `onLoad` callback to be synchronous (`(data, name) => { void handleFloppyAFromServer(data, name); }`).

Apply the same pattern for **Floppy B**, **Hard Drive C**, and **CD-ROM**.

The `<Group>` for each row becomes:
```tsx
<Group gap="xs">
    {/* Existing FileButton */}
    <ImageLibraryPicker
        driveType="floppy"   // or "hdd" / "cdrom"
        onLoad={(data, name) => { void handleFloppyAFromServer(data, name); }}
    />
    {/* Existing eject / manage / download icons */}
</Group>
```

### Handler summaries

| Drive | `driveType` prop | WASM call |
|-------|-----------------|-----------|
| Floppy A | `"floppy"` | `computer.load_floppy(0, data)` |
| Floppy B | `"floppy"` | `computer.load_floppy(1, data)` |
| Hard Drive C | `"hdd"` | `computer.set_hard_drive(0x80, data)` |
| CD-ROM | `"cdrom"` | `computer.load_cdrom(0, data)` |

---

## Implementation Steps

1. **Create `wasm/www/public/images.json`** with empty arrays.
2. **Create `wasm/www/src/imageLibrary.ts`** with `fetchImagesLibrary` and `fetchImageData`.
3. **Create `wasm/www/src/components/ImageLibraryPicker.tsx`** component.
4. **Edit `wasm/www/src/components/DriveControl.tsx`**:
   - Import `ImageLibraryPicker`.
   - Add server-load handlers for each drive (Floppy A, B, HDD, CD-ROM).
   - Insert `<ImageLibraryPicker>` in each drive's `<Group>` row.
5. **Run `./scripts/pre-commit.sh`** to lint/build and verify no errors.

---

## Notes

- `ImageLibraryPicker` returns `null` when the images list for that type is empty, so the
  UI is unchanged when `images.json` has no entries for a given drive type.
- The component renders only after the first render (useEffect), so there is no flash.
- `fetchImagesLibrary` is called independently per component instance but the module-level
  cache ensures only one network request is made.
- Setting `floppyAFile.value = new File([data], name)` means the remount logic in
  `useSignalEffect` will re-fetch the binary from the in-memory `File` object on reset
  (FileReader over a `File` constructed from `Uint8Array` works correctly in browsers).
- Images served via Vite dev server or production nginx must be accessible at their `url`
  paths. For dev, place files under `wasm/www/public/` and reference them as `/images/...`.

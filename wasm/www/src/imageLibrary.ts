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
    if (cache) {
        return cache;
    }
    fetchPromise ??= fetch('/images.json')
        .then((r) => r.json() as Promise<unknown>)
        .then((data) => {
            const d = data as { floppy?: unknown; hdd?: unknown; cdrom?: unknown };
            cache = {
                floppy: Array.isArray(d.floppy) ? (d.floppy as DiskImage[]) : [],
                hdd: Array.isArray(d.hdd) ? (d.hdd as DiskImage[]) : [],
                cdrom: Array.isArray(d.cdrom) ? (d.cdrom as DiskImage[]) : [],
            };
            return cache;
        })
        .catch(() => {
            // If the file is missing or malformed, silently return empty library
            cache = { floppy: [], hdd: [], cdrom: [] };
            return cache;
        });
    return fetchPromise;
}

// Fetches a binary image from a URL entry (resolves relative URLs against page origin)
export async function fetchImageData(url: string): Promise<Uint8Array> {
    const resolved = new URL(url, window.location.href).href;
    const response = await fetch(resolved);
    if (!response.ok) {
        throw new Error(`HTTP ${response.status} fetching ${resolved}`);
    }
    const buffer = await response.arrayBuffer();
    return new Uint8Array(buffer);
}

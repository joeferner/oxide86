#!/usr/bin/env python3
"""
Analyze oxide86.log and recreate the last screen image from graphics operations.
"""

import re
import sys
from pathlib import Path
from PIL import Image, ImageDraw

# CGA palette colors (standard CGA palette 1: black, cyan, magenta, white)
CGA_PALETTE = [
    (0, 0, 0),        # 0: Black
    (0, 255, 255),    # 1: Cyan
    (255, 0, 255),    # 2: Magenta
    (255, 255, 255),  # 3: White
]

# VGA/EGA standard palette (16 colors)
EGA_PALETTE = [
    (0, 0, 0),        # 0: Black
    (0, 0, 170),      # 1: Blue
    (0, 170, 0),      # 2: Green
    (0, 170, 170),    # 3: Cyan
    (170, 0, 0),      # 4: Red
    (170, 0, 170),    # 5: Magenta
    (170, 85, 0),     # 6: Brown
    (170, 170, 170),  # 7: Light Gray
    (85, 85, 85),     # 8: Dark Gray
    (85, 85, 255),    # 9: Light Blue
    (85, 255, 85),    # 10: Light Green
    (85, 255, 255),   # 11: Light Cyan
    (255, 85, 85),    # 12: Light Red
    (255, 85, 255),   # 13: Light Magenta
    (255, 255, 85),   # 14: Yellow
    (255, 255, 255),  # 15: White
]


def ega_formula_color(i):
    """Compute VGA DAC RGB (6-bit) for EGA 64-color palette index i.

    Each 6-bit index encodes two intensity bits per channel:
      R = bit2 * 0x2A + bit5 * 0x15
      G = bit1 * 0x2A + bit4 * 0x15
      B = bit0 * 0x2A + bit3 * 0x15
    6-bit values are scaled to 8-bit via (v << 2) | (v >> 4).
    """
    r6 = ((i >> 2) & 1) * 0x2A + ((i >> 5) & 1) * 0x15
    g6 = ((i >> 1) & 1) * 0x2A + ((i >> 4) & 1) * 0x15
    b6 = (i & 1) * 0x2A + ((i >> 3) & 1) * 0x15
    return (
        (r6 << 2) | (r6 >> 4),
        (g6 << 2) | (g6 >> 4),
        (b6 << 2) | (b6 >> 4),
    )


class ScreenRecreator:
    def __init__(self):
        self.mode = None
        self.width = 0
        self.height = 0
        self.scale = 2  # Scale factor for output image
        self.pixels = None
        self.palette = None
        # VGA DAC: entries 0-15 = EGA 16-color text palette, entries 16-63 =
        # EGA 64-color formula (matches what the IBM VGA BIOS programs on mode set).
        self.vga_dac = [(0, 0, 0)] * 256
        for i, color in enumerate(EGA_PALETTE):
            self.vga_dac[i] = color
        for i in range(16, 64):
            self.vga_dac[i] = ega_formula_color(i)
        # AC palette: maps EGA pixel values 0-15 to VGA DAC indices.
        # Default is identity; EGA programs override via port 0x3C0.
        self.ac_palette = list(range(16))
        # EGA plane buffers: 4 planes, each 40 bytes wide × 200 rows = 8000 bytes
        self.ega_planes = [[0] * 8000 for _ in range(4)]

    def set_vga_dac(self, index, r, g, b):
        """Set VGA DAC palette entry (6-bit RGB values)."""
        # Convert 6-bit (0-63) to 8-bit (0-255)
        r8 = (r << 2) | (r >> 4)
        g8 = (g << 2) | (g >> 4)
        b8 = (b << 2) | (b >> 4)
        self.vga_dac[index] = (r8, g8, b8)

    def set_ac_palette(self, ac_values):
        """Set AC palette registers."""
        self.ac_palette = ac_values.copy()
        print(f"AC Palette: {self.ac_palette[:4]}")

    def update_palette(self):
        """Update working palette based on mode, VGA DAC, and AC palette."""
        if self.mode is None:
            return
        if "Graphics320x200x256" in self.mode:
            # VGA mode 13h: 256 color index → VGA DAC directly
            self.palette = self.vga_dac  # full 256-entry DAC
        elif "Graphics320x200" in self.mode and "x16" not in self.mode:
            # CGA 4-color mode: use AC palette to map to VGA DAC
            self.palette = [
                self.vga_dac[self.ac_palette[0]],
                self.vga_dac[self.ac_palette[1]],
                self.vga_dac[self.ac_palette[2]],
                self.vga_dac[self.ac_palette[3]],
            ]
            print(f"Effective CGA palette (via AC {self.ac_palette[:4]}):")
            for i in range(4):
                ac_idx = self.ac_palette[i]
                rgb = self.vga_dac[ac_idx]
                print(f"  Pixel {i} → AC[{i}]={ac_idx} → VGA_DAC[{ac_idx}] = RGB{rgb}")
        elif "Graphics640x200" in self.mode:
            self.palette = [self.vga_dac[0], self.vga_dac[15]]  # Black and white
        elif "Graphics320x200x16" in self.mode:
            # EGA 16-color: pixel value (0-15) → AC palette → VGA DAC index → color
            self.palette = [self.vga_dac[self.ac_palette[i]] for i in range(16)]
            print(f"Effective EGA palette (via AC palette):")
            for i in range(16):
                ac_idx = self.ac_palette[i]
                rgb = self.vga_dac[ac_idx]
                print(f"  Pixel {i} → AC[{i}]={ac_idx} → VGA_DAC[{ac_idx}] = RGB{rgb}")
        else:
            self.palette = CGA_PALETTE

    def set_mode(self, mode_str):
        """Set video mode and initialize pixel buffer."""
        self.mode = mode_str

        if "Graphics320x200x256" in mode_str:
            self.width = 320
            self.height = 200
        elif "Graphics320x200x16" in mode_str:
            self.width = 320
            self.height = 200
        elif "Graphics320x200" in mode_str:
            self.width = 320
            self.height = 200
        elif "Graphics640x200" in mode_str:
            self.width = 640
            self.height = 200
        else:
            print(f"Unknown mode: {mode_str}")
            return

        # Initialize pixel buffer with black
        self.pixels = [[[0, 0, 0] for _ in range(self.width)] for _ in range(self.height)]
        # Reset EGA planes
        self.ega_planes = [[0] * 8000 for _ in range(4)]
        print(f"Set video mode: {mode_str} ({self.width}x{self.height})")
        self.update_palette()

    def write_2bpp(self, x, y, value):
        """Write a byte in 2bpp format (4 pixels, 2 bits each)."""
        if self.pixels is None or y >= self.height:
            return

        # Extract 4 pixels from the byte (2 bits per pixel)
        for i in range(4):
            px = x + i
            if px >= self.width:
                break
            # Extract 2-bit color index (highest bits first)
            color_index = (value >> (6 - i * 2)) & 0x03
            if color_index < len(self.palette):
                self.pixels[y][px] = list(self.palette[color_index])

    def write_1bpp(self, x, y, value):
        """Write a byte in 1bpp format (8 pixels, 1 bit each)."""
        if self.pixels is None or y >= self.height:
            return

        # Extract 8 pixels from the byte (1 bit per pixel)
        for i in range(8):
            px = x + i
            if px >= self.width:
                break
            # Extract 1-bit color index (highest bit first)
            color_index = (value >> (7 - i)) & 0x01
            if color_index < len(self.palette):
                self.pixels[y][px] = list(self.palette[color_index])

    def write_8bpp(self, x, y, value):
        """Write a single pixel in 8bpp format (VGA mode 13h, 256 colors)."""
        if self.pixels is None or y >= self.height or x >= self.width:
            return
        color = self.vga_dac[value] if value < len(self.vga_dac) else (0, 0, 0)
        self.pixels[y][x] = list(color)

    def write_ega_plane(self, plane, x, y, value):
        """Write a byte to an EGA plane and update the pixel buffer.

        Each byte covers 8 pixels; x is the leftmost pixel of the group.
        Bits are combined across all 4 planes to produce a 4-bit color index.
        """
        if self.pixels is None or plane >= 4:
            return
        # Store in plane buffer (offset = y * 40 + x // 8)
        byte_x = x // 8
        offset = y * 40 + byte_x
        if offset < 8000:
            self.ega_planes[plane][offset] = value

        # Reconstruct the 8 pixels at this byte position from all 4 planes
        for bit in range(8):
            px = x + bit
            if px >= self.width or y >= self.height:
                continue
            color_index = 0
            for p in range(4):
                byte_offset = y * 40 + px // 8
                if byte_offset < 8000:
                    if self.ega_planes[p][byte_offset] & (0x80 >> (px % 8)):
                        color_index |= (1 << p)
            if self.palette and color_index < len(self.palette):
                self.pixels[y][px] = list(self.palette[color_index])

    def process_write(self, x, y, value, format_str, plane=None):
        """Process a graphics write operation."""
        if format_str == "2bpp":
            self.write_2bpp(x, y, value)
        elif format_str == "1bpp":
            self.write_1bpp(x, y, value)
        elif format_str == "8bpp":
            self.write_8bpp(x, y, value)
        elif format_str.startswith("ega_p") and plane is not None:
            self.write_ega_plane(plane, x, y, value)
        else:
            print(f"Unknown format: {format_str}")

    def save_image(self, output_path):
        """Save the pixel buffer to an image file."""
        if self.pixels is None:
            print("No pixel data to save")
            return

        # Create image
        img = Image.new('RGB', (self.width, self.height))

        # Set pixels
        for y in range(self.height):
            for x in range(self.width):
                img.putpixel((x, y), tuple(self.pixels[y][x]))

        # Scale up for better visibility
        if self.scale > 1:
            img = img.resize((self.width * self.scale, self.height * self.scale), Image.NEAREST)

        img.save(output_path)
        print(f"Image saved to: {output_path}")


def find_last_screen(log_path):
    """Find the last screen in the log file."""
    mode_pattern = re.compile(r'Video mode set to 0x[0-9A-Fa-f]+ \(([^)]+)\)')
    # Write Mode 0/1 format: value=0xVV (ega_pN) or value=0xVV (2bpp|1bpp|8bpp)
    write_pattern = re.compile(
        r'Graphics write: offset=0x[0-9A-Fa-f]+ \(x=(\d+), y=(\d+)\), value=0x([0-9A-Fa-f]+) \((\w+)\)'
    )
    # Write Mode 2 format: color=0xC mask=0xMM -> pN = 0xVV (one line per plane)
    write_mode2_pattern = re.compile(
        r'Graphics write: offset=0x[0-9A-Fa-f]+ \(x=(\d+), y=(\d+)\), color=0x[0-9A-Fa-f]+ '
        r'mask=0x[0-9A-Fa-f]+ -> p(\d+) = 0x([0-9A-Fa-f]+)'
    )
    vga_dac_pattern = re.compile(
        r'VGA DAC: Setting palette\[(\d+)\] = RGB\((\d+), (\d+), (\d+)\)'
    )
    # Matches individual AC register writes: "AC Palette: Register N = V (DAC index)"
    ac_register_pattern = re.compile(
        r'AC Palette: Register (\d+) = (\d+) \(DAC index\)'
    )

    recreator = ScreenRecreator()
    last_mode_line = None
    last_ac_palette = list(range(16))  # identity default
    write_count = 0

    print(f"Reading log file: {log_path}")

    # Read the entire file and find the last mode switch
    with open(log_path, 'r') as f:
        lines = f.readlines()

    print(f"Total lines: {len(lines)}")

    # Scan forward to find last mode switch and track AC palette state
    for i in range(len(lines)):
        # Track individual AC register writes
        match = ac_register_pattern.search(lines[i])
        if match:
            reg = int(match.group(1))
            val = int(match.group(2))
            if 0 <= reg < 16:
                last_ac_palette[reg] = val

        # Track mode switches (reset AC to identity on mode change)
        match = mode_pattern.search(lines[i])
        if match:
            last_mode_line = i
            mode_str = match.group(1)
            last_ac_palette = list(range(16))

    if last_mode_line is None:
        print("No mode switch found in log file")
        return None

    # Set mode and apply last known AC palette
    recreator.set_mode(mode_str)
    recreator.set_ac_palette(last_ac_palette)
    recreator.update_palette()
    print(f"Found last mode switch at line {last_mode_line}: {mode_str}")

    # Process all VGA DAC, AC palette, and graphics writes after the last mode switch
    palette_updated = False
    for i in range(last_mode_line + 1, len(lines)):
        # Check for VGA DAC updates
        match = vga_dac_pattern.search(lines[i])
        if match:
            index = int(match.group(1))
            r = int(match.group(2))
            g = int(match.group(3))
            b = int(match.group(4))
            recreator.set_vga_dac(index, r, g, b)
            palette_updated = True
            continue

        # Check for individual AC register updates
        match = ac_register_pattern.search(lines[i])
        if match:
            reg = int(match.group(1))
            val = int(match.group(2))
            if 0 <= reg < 16:
                recreator.ac_palette[reg] = val
                palette_updated = True
            continue

        # Check for Write Mode 2 plane writes (color=... mask=... -> pN = 0xVV)
        match = write_mode2_pattern.search(lines[i])
        if match:
            if palette_updated:
                recreator.update_palette()
                palette_updated = False
            x = int(match.group(1))
            y = int(match.group(2))
            plane = int(match.group(3))
            value = int(match.group(4), 16)
            recreator.write_ega_plane(plane, x, y, value)
            write_count += 1
            if write_count % 10000 == 0:
                print(f"Processed {write_count} writes...")
            continue

        # Check for Write Mode 0/1 graphics writes
        match = write_pattern.search(lines[i])
        if match:
            # Update palette if needed before first write
            if palette_updated:
                recreator.update_palette()
                palette_updated = False

            x = int(match.group(1))
            y = int(match.group(2))
            value = int(match.group(3), 16)
            format_str = match.group(4)

            # Parse plane number for EGA plane writes (format: ega_p0, ega_p1, etc.)
            plane = None
            if format_str.startswith("ega_p"):
                try:
                    plane = int(format_str[5:])
                except ValueError:
                    pass

            recreator.process_write(x, y, value, format_str, plane)
            write_count += 1

            if write_count % 10000 == 0:
                print(f"Processed {write_count} writes...")

    print(f"Total graphics writes processed: {write_count}")

    return recreator


def main():
    log_path = Path(__file__).parent.parent / "oxide86.log"
    output_path = Path(__file__).parent.parent / "screen_output.png"

    if len(sys.argv) > 1:
        log_path = Path(sys.argv[1])

    if len(sys.argv) > 2:
        output_path = Path(sys.argv[2])

    if not log_path.exists():
        print(f"Log file not found: {log_path}")
        sys.exit(1)

    recreator = find_last_screen(log_path)

    if recreator:
        recreator.save_image(output_path)
        print(f"\nSuccess! Image saved to: {output_path}")
    else:
        print("Failed to recreate screen")
        sys.exit(1)


if __name__ == "__main__":
    main()

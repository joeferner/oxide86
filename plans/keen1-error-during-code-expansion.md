# Commander Keen 1 - "Error during code expansion!" Investigation

## Status: PARTIALLY RESOLVED

The game works when run via `--boot --floppy-a examples/keen1.img`. The error was
observed in a previous session and does not appear in the current log.

## Original Error

When running Commander Keen 1, the game displays "Error during code expansion!" then
continues running (falling back to phase 2).

## LZEXE 0.91 Compression Analysis

keen1.exe (51190 bytes) is LZEXE 0.91 compressed ("LZ91" at file offset 0x1C).

EXE Header:
- CS offset: 0x0C66, IP: 0x000E (entry at file offset 0xC68E)
- SS offset: 0x1892, SP: 0x0080
- Min para: 0x119F, Max para: 0xFFFF

LZEXE Stub (374 bytes at file offset 0xC680):
1. Entry CS:000E: copies 374-byte stub backward (STD) to ES = CS + 0x0C14
2. RETF to relocated stub at (CS+0x0C14):0x002B
3. Big copy: 0xC660 bytes backward from load_seg:0 to (load_seg+0x0C14):0
   - After copy: DS = load_seg+0x0C14 (compressed source), ES = load_seg (output)
4. CLD; XOR SI,SI; XOR DI,DI - forward decompression
5. Decompressor loop at 0x0069:
   - 16-bit control words in BP, bit counter in DX
   - CF=1 -> literal: MOVSB
   - CF=0, next_bit=0 -> short match: 2 bits length + 1 byte offset (BX=0xFF:al)
   - CF=0, next_bit=1 -> long match: 2-byte token, BH |= 0xE0, BH>>=3
     - if length==0: read extra byte; if 0=end, if 1=segment change
   - Back-ref copy: MOV AL,[ES:BX+DI]; STOSB; LOOP
6. Segment change (AL==1) at 0x00D1:
   - DI = (DI & 0xF) + 0x2000; ES = ES + (old_DI>>4) - 0x200
   - SI = (SI & 0xF);          DS = DS + (old_SI>>4)
   - Preserves physical address, normalizes DI into 0x2000..0x200F range
7. End of data (AL==0): jumps to relocation at 0xFC
8. Relocation at 0xFC:
   - PUSH CS; POP DS (DS = stub segment)
   - POP BX -> PSP segment; ADD BX,0x10 -> BX = load_seg (relocation base)
   - Reads relocation table from stub+0x158, adds load_seg to each entry

All LZEXE-relevant CPU instructions were verified correct:
- SHR BP,1 (D1ED): correctly shifts LSB into CF
- RCL CX,1 (D1D1): correctly shifts CF into CX
- LODSW/LODSB: correct DS:SI with segment override
- STOSB: correct ES:DI
- MOVSB/MOVSW: correct with direction flag
- MOV DS,AX / MOV ES,AX: immediate effect
- MOV AL,[ES:BX+DI] (26 8A 01): segment override correctly uses ES
- decode_modrm: [BX+DI] (R/M=001) correctly uses DS default, ES override
- memory write_u8: no address filtering that would block writes
- physical_address: (seg<<4)+offset correct

## Commander Keen's Internal Decompressor

After LZEXE, Commander Keen has its own state-machine decompressor at 0x0F65:6B6E:
- Reads from compressed chunk at 0x8CAD:0x0004 (physical 0x8CAD4)
- State table at 0x8A39:0x0004 (physical 0x8A394)
- State table root state = 0x05B6 at physical 0x8AF00
- Watchdog fires after 0x0FA0 (4000) stuck iterations -> "Error during code expansion!"
- Error: state 0x05B6 -> reads 0x00 from physical 0x8D08A -> stays at 0x05B6 -> loop

## Two-Stage Architecture

1. LZEXE decompresses the EXE image (game code + state tables + static data)
2. Commander Keen's loader loads .CK1 files from disk into memory, then decompresses
   them with its internal Carmack/Huffman decompressor

keen1.img contains all required .CK1 files:
- EGAHEAD.CK1 (15568), EGALATCH.CK1 (57065), EGASPRIT.CK1 (17633)
- SOUNDS.CK1 (8898), LEVEL01-LEVEL16.CK1, etc.

## Hypothesis for Previous Error

Most likely cause: running --program keen1.exe without .CK1 files accessible.
Commander Keen's loader couldn't find EGAHEAD.CK1 etc. -> zero data in compressed
buffer -> state machine reads zeros -> infinite loop -> watchdog -> error.

## Current Status

Game works correctly when booted from disk image:
  cargo run -p emu86-native-gui -- --boot --floppy-a examples/keen1.img

Most recent log shows:
- INT 2Fh hook at 0x0E28:1076 running correctly
- Normal keyboard input polling (INT 16h) reached
- No "Error during code expansion!"

## Remaining Issues

INT 10h AH=12h BL=10h (Get EGA Info) - warned as unimplemented (3 times at startup).
Commander Keen calls this to detect EGA. Should return:
- BL=0x10 (EGA installed), BH=0 (color mode), CX=EGA feature bits

## Next Steps (If Error Returns)

1. Confirm running from --boot --floppy-a keen1.img (not --program keen1.exe)
2. Add write-trace in memory.rs write_u8():
     if addr >= 0x8CAD0 && addr < 0x8CAF0 {
         log::debug!("KEEN: Write {:05X} = {:02X}", addr, value);
     }
3. Enable exec logging before LZEXE runs to trace full decompression
4. Implement INT 10h AH=12h BL=10h (Get EGA info)

# Modem Emulation Plan

## Background and Scope

This plan adds Hayes-compatible modem emulation as a pluggable `ComPortDevice` attached to any COM port. The modem bridges the guest's serial port to real TCP connections on the host, enabling vintage DOS terminal software (Telix, Procomm Plus, Qmodem, MS-DOS Kermit) to connect to real servers and BBS systems.

### Architecture overview

```
DOS software
    ↕ INT 14h / BIOS serial
UART (8250 emulation)
    ↕ ComPortDevice trait
SerialModem struct
    ↕ mpsc channels
TCP bridge thread
    ↕ std::net::TcpStream
Real TCP host
```

The `SerialModem` implements `ComPortDevice` (defined in `core/src/devices/uart.rs`). It owns a background `std::thread` that owns the `TcpStream`. Communication between the device and the thread uses `std::sync::mpsc` channels. The device is `Arc<RwLock<SerialModem>>`, matching the existing pattern for serial devices.

### Development approach

Each coding phase follows the same pattern as the Sound Blaster plan:
1. Write an assembly test in `core/src/test_data/devices/modem/`
2. Write the Rust test in `core/src/tests/devices/modem.rs`
3. Implement just enough to pass those tests

Assembly tests exit with code `0` on pass, non-zero for specific failures.

---

## Phase 1 — Stub Modem Device + CLI Wiring

**Goal:** Attach a modem stub to a COM port so that early integration testing can happen immediately. The device accepts AT commands and responds with `OK` / `ERROR`. No TCP connection yet.

### 1.1 Test first

**`core/src/test_data/devices/modem/at_basic.asm`**

```
; Initialize COM1 at 1200 baud
; Write "AT\r" to the UART
; Read response — expect "AT\r\nOK\r\n" (echo + result)
; Exit 0 on match, 1 on timeout/mismatch
```

Rust test in `core/src/tests/devices/modem.rs` runs this assembly and asserts exit code 0.

### 1.2 New file: `core/src/devices/modem.rs`

Implement `SerialModem` with:

```rust
pub struct SerialModem {
    state: ModemState,       // CommandMode | DataMode | Dialing | Connected
    cmd_buf: String,         // accumulates chars until CR in command mode
    rx_queue: VecDeque<u8>,  // bytes waiting for the UART to read
    irq_pending: bool,
    echo: bool,              // ATE1 (default on)
    verbose: bool,           // ATV1 (default on) — text vs numeric result codes
    quiet: bool,             // ATQ0 (default off) — suppress result codes
    // phase 3 additions (None until TCP connected):
    tx_send: Option<mpsc::Sender<u8>>,
    rx_recv: Option<mpsc::Receiver<u8>>,
    dcd: bool,               // raised when TCP connected
}
```

**AT commands implemented in phase 1** (all others respond `ERROR`):
- `AT` → `OK`
- `ATZ` → reset registers, `OK`
- `AT&F` → factory defaults, `OK`
- `ATE0` / `ATE1` → echo off/on
- `ATV0` / `ATV1` → numeric/verbose result codes
- `ATQ0` / `ATQ1` → result codes on/off

Result code strings (verbose mode):
```
OK        CONNECT    RING
NO CARRIER  ERROR   NO DIALTONE
BUSY      NO ANSWER  CONNECT 2400 …
```

Numeric equivalents: 0, 1, 2, 3, 4, 6, 7, 8, 10 …

### 1.3 CLI — `native-common/src/cli.rs`

Add `modem` as a valid `--com1` / `--com2` / `--com3` / `--com4` option alongside the existing `mouse` and `loopback` values.

Add new flags:
```
--modem-phonebook <PATH>     JSON phonebook file (see Phase 2)
--modem-com <PORT_NUM>       Which COM port gets the modem (default: 1)
```

Update `create_com_device()` in `native-common/src/lib.rs` to construct `SerialModem` when the device name is `"modem"`.

### 1.4 WASM — `wasm/src/lib.rs`

Add `modem_com: Option<u32>` and `modem_phonebook: Option<String>` to `WasmComputerConfig`. The WASM modem can only connect to WebSocket endpoints (phase 3 extension); in phase 1 it is wired as a command-mode stub only so the AT parser can be tested.

### 1.5 Register in `core/src/devices/mod.rs`

```rust
pub mod modem;
```

No changes to `Bus` or `Pic` needed — the modem routes through the existing UART device.

---

## Phase 2 — AT Command Parser + Phonebook Configuration

**Goal:** Full Hayes AT command parsing and a phonebook config that maps short dial strings to `host:port` pairs.

### 2.1 Tests

**`core/src/test_data/devices/modem/at_dial_reject.asm`**
- Sends `ATDT555\r`
- Expects `NO DIALTONE\r\n` (no phonebook entry, no TCP yet)
- Exit 0 on correct response

**`core/src/test_data/devices/modem/at_hangup.asm`**
- Sends `ATDT0\r`, waits for result, sends `ATH\r`, expects `OK\r\n`

### 2.2 Commands added in phase 2

| Command | Action |
|---------|--------|
| `ATDT<num>` | Dial tone — look up `<num>` in phonebook |
| `ATDP<num>` | Dial pulse — same as `ATDT` |
| `ATD<num>` | Dial — same |
| `ATH` / `ATH0` | Hang up, set DCD low, return `NO CARRIER` |
| `ATH1` | Go off-hook (no-op in phase 2, handled in phase 3) |
| `ATA` | Answer incoming call (stub → `ERROR` until phase 3) |
| `ATS0=N` | Auto-answer after N rings (store for phase 3) |
| `AT+++ ` | Escape: switch from data mode back to command mode |
| `ATI` | Return modem identity string |
| `AT?` | Return current register value |

### 2.3 Phonebook file format

`phonebook.json`:
```json
{
  "0":   "127.0.0.1:2323",
  "555": "bbs.example.com:23",
  "1":   "192.168.1.10:513"
}
```

Keys are the dial strings the DOS software sends after `ATDT`. Values are `host:port`. Entries with no phonebook match fall back to parsing the dial string directly as `aaa.bbb.ccc.ddd/pppp` (slash-separated IP and port) or `host:port` with a leading `+` for literal addresses:

```
ATDT+192.168.1.1:23     → connect to 192.168.1.1:23 directly
ATDT555                 → phonebook lookup → bbs.example.com:23
```

### 2.4 `ModemPhonebook` struct

New file `core/src/devices/modem_phonebook.rs`:

```rust
pub struct ModemPhonebook {
    entries: HashMap<String, String>,  // dial string → "host:port"
}
impl ModemPhonebook {
    pub fn from_file(path: &Path) -> Result<Self>;
    pub fn from_json(json: &str) -> Result<Self>;
    pub fn lookup(&self, number: &str) -> Option<&str>;
    pub fn resolve(&self, number: &str) -> Option<SocketAddr>;
}
```

The phonebook is loaded at startup and passed into `SerialModem::new()`.

---

## Phase 3 — TCP Connection Bridge

**Goal:** Wire `ATDT` to a real TCP connection. Bytes in data mode flow between the UART and the socket.

### 3.1 Tests

**`core/src/test_data/devices/modem/tcp_echo.asm`**
- Sends `ATDT0\r` (phonebook entry `0` → `127.0.0.1:<TEST_PORT>`)
- Waits for `CONNECT\r\n`
- Sends a known string (e.g., `Hello\r\n`)
- Reads back the echo
- Exit 0 on match

The Rust test in `core/src/tests/devices/modem.rs` spins up a `TcpListener` on a random port before running the assembly. The test server echoes every byte back.

### 3.2 Connection state machine

```
IDLE ──ATDT──► DIALING ──TCP connect ok──► CONNECTED
  ▲                │                          │
  └── ATH / DCD drop ◄─────────── TCP close ─┘
  
CONNECTED ──+++──► COMMAND_MODE (DCD stays high)
COMMAND_MODE ──ATH──► IDLE (TCP close, DCD low)
```

### 3.3 Threading model

When `ATDT` resolves to a `SocketAddr`:

```rust
let (host_tx, modem_rx) = mpsc::channel::<u8>();  // host → modem RX
let (modem_tx, host_rx) = mpsc::channel::<u8>();  // modem TX → host

std::thread::spawn(move || {
    match TcpStream::connect(addr) {
        Ok(stream) => {
            // send CONNECT result back on host_tx
            // spawn reader thread: stream.read → host_tx
            // loop: host_rx.recv → stream.write
        }
        Err(_) => {
            // send NO CARRIER on host_tx
        }
    }
});

self.tx_send = Some(modem_tx);
self.rx_recv = Some(modem_rx);
self.state = ModemState::Dialing;
```

The reader sub-thread owns a `TcpStream::try_clone()` and pushes received bytes into `host_tx`. The main bridge thread owns the write half. On TCP close, the reader sends a sentinel value and the bridge thread queues `NO CARRIER` + drops DCD.

### 3.4 Modem status lines

| Line | Condition |
|------|-----------|
| DSR (bit 5) | Always high when `SerialModem` is active |
| DCD (bit 7) | High when TCP socket is open |
| CTS (bit 4) | High when not dialing (ready to accept TX bytes) |
| RI  (bit 6) | Pulse when incoming connection arrives (future server mode) |

### 3.5 Escape sequence (+++)

Guard-time escape: when the guest writes `+` three consecutive times with no other characters, and then pauses for the guard time (configurable via `ATS12`, default ~1 second of emulated time / ~50ms wall-clock), switch from data mode to command mode without dropping the TCP connection. DCD stays high.

---

## Phase 4 — Simple Echo Server Example

**Goal:** A working example that demonstrates the modem end-to-end, including a real TCP server and setup instructions.

### 4.1 Example directory layout

```
examples/modem/
├── README.md           ← setup instructions
├── modem_demo.asm      ← DOS assembly demo program
├── modem_demo.com      ← compiled (by build.rs)
└── echo_server/
    ├── Cargo.toml      ← tiny standalone Rust binary
    └── src/
        └── main.rs     ← TCP echo server on port 2323
```

### 4.2 `modem_demo.asm`

The assembly program:
1. Initializes COM1 at 1200 baud via INT 14h
2. Sends `ATZ\r` and waits for `OK`
3. Sends `ATDT0\r` (phonebook entry `0` → echo server)
4. Waits for `CONNECT`
5. Sends `Hello, modem world!\r\n`
6. Reads back the echo and displays it on screen
7. Sends `+++` pause `ATH\r` to hang up
8. Displays `Done.` and exits

### 4.3 Echo server (`echo_server/src/main.rs`)

Minimal TCP echo server: bind to `0.0.0.0:2323`, accept connections, echo every byte back. Prints each connection to stdout. No external dependencies.

### 4.4 README.md (setup instructions)

```markdown
## Modem Echo Demo

### Prerequisites
- oxide86 built with modem support
- nasm (to compile the .asm — or use the precompiled .com)

### Steps

1. Start the echo server:
   cd examples/modem/echo_server
   cargo run

2. Create phonebook.json:
   echo '{"0":"127.0.0.1:2323"}' > phonebook.json

3. Start the emulator:
   cargo run -p oxide86-cli -- \
     --com1 modem \
     --modem-phonebook phonebook.json \
     examples/modem/modem_demo.com

4. The program connects, sends a greeting, and echoes it back.
```

---

## Phase 5 — DOS Terminal Software Setup

**Goal:** Run real vintage DOS terminal software (Telix, Procomm Plus, Qmodem, or MS-DOS Kermit) in the emulator against the modem device. This phase is documentation + validation only — no emulator code changes.

### 5.1 Recommended software

| Software | Notes |
|----------|-------|
| **Telix 3.51** | Most widely available shareware; excellent COM1 config |
| **Procomm Plus 2.4** | Commercial; scripts (ASPECT) are useful |
| **Qmodem 4.6** | Shareware, ANSI BBS support |
| **MS-DOS Kermit 3.15** | Freely redistributable; good for scripted transfers |

All are available via archive.org's MS-DOS software library.

### 5.2 Creating a floppy image with the software

```bash
# Create a 1.44 MB FAT12 floppy image
dd if=/dev/zero of=telix.img bs=512 count=2880
mformat -i telix.img ::
# Mount and copy Telix files
mcopy -i telix.img TELIX.EXE ::
mcopy -i telix.img TELIX.FON ::
```

### 5.3 Running the emulator

```bash
cargo run -p oxide86-cli -- \
  --boot \
  --floppy-a msdos6.img \          # bootable DOS floppy
  --floppy-b telix.img \           # Telix on B:
  --com1 modem \
  --modem-phonebook phonebook.json
```

### 5.4 Configuring Telix for COM1

Inside Telix: `Alt-O` → `Modem and Dialing` → set port to `COM1`, baud `1200` (or `9600` if supported). The emulated modem accepts any baud rate — the UART divisor is ignored for TCP throughput.

Create a Telix dialing directory entry:
- Name: `Echo Test`
- Number: `0` (phonebook key)
- Protocol: `None` (raw connection)

### 5.5 Validation checklist

- [ ] Telix connects to local echo server
- [ ] ANSI color codes render correctly
- [ ] Zmodem file transfer works (requires extending the modem to pass binary cleanly)
- [ ] Hang up via `Alt-H` (sends `ATH`)
- [ ] Reconnect works after hang up

---

## Phase 6 — Docker Compose: tcpser + WWIV BBS

**Goal:** A self-contained Docker Compose setup that runs a classic BBS reachable via the emulated modem.

### 6.1 Directory layout

```
docker/bbs/
├── docker-compose.yml
├── tcpser/
│   └── Dockerfile         ← builds tcpser from source
├── wwiv/
│   └── Dockerfile         ← WWIV BBS image
│   └── init.sh            ← initial sysop setup
└── README.md
```

### 6.2 `docker-compose.yml`

```yaml
version: "3.9"
services:
  tcpser:
    build: ./tcpser
    ports:
      - "2323:2323"         # modem-side port (phonebook points here)
    environment:
      BBS_HOST: wwiv
      BBS_PORT: 23
    depends_on:
      - wwiv

  wwiv:
    build: ./wwiv
    ports:
      - "23:23"             # internal telnet port
    volumes:
      - wwiv_data:/opt/wwiv/data

volumes:
  wwiv_data:
```

**tcpser** listens on port 2323, presents a Hayes modem interface over TCP, and proxies to the WWIV telnet port. This means the DOS modem software goes through a complete dial→connect→data cycle that tcpser manages.

**WWIV** is the BBS itself; the image runs `wwivd` and pre-configures a single sysop account.

### 6.3 Phonebook entry

```json
{
  "1": "127.0.0.1:2323"
}
```

Dial `1` from any DOS terminal software to reach the BBS.

### 6.4 README.md (setup instructions)

```markdown
## WWIV BBS via tcpser

### Start the stack
cd docker/bbs
docker compose up -d

### Phonebook
Add to phonebook.json: {"1":"127.0.0.1:2323"}

### Connect
cargo run -p oxide86-cli -- \
  --com1 modem \
  --modem-phonebook phonebook.json \
  --boot --floppy-a msdos.img --floppy-b telix.img

Inside Telix: dial "1" → WWIV BBS login screen appears.

### Stop
docker compose down
```

### 6.5 Notes on tcpser

`tcpser` (by Jim Meritt) wraps a TCP connection in a Hayes command emulation layer. The emulated modem in the emulator dials → tcpser answers → tcpser forwards the data to WWIV. This is the standard way vintage DOS BBS software reaches modern TCP servers. `tcpser` source: https://github.com/jmeberlein/tcpser

---

## Phase 7 — Classic Modem Sounds (Optional)

**Goal:** Play sampled modem audio during connection phases to recreate the classic DOS feel.

### 7.1 Sounds to include

| Phase | Sound | Duration |
|-------|-------|----------|
| Off-hook / dial tone | 425 Hz sine tone | Until dialing starts |
| DTMF dialing | Per-digit dual tones (697+1209 Hz for `1`, etc.) | ~100ms per digit |
| Ringing | 25 Hz modulated tone | 2 s on / 4 s off |
| Carrier handshake | Classic V.32 chirp sequence | ~30 s |
| Data transfer | White noise modulated by line activity | Ongoing |
| Hang-up / carrier lost | Short silence + click | Once |

### 7.2 Implementation approach

New file `core/src/devices/modem_audio.rs`:

```rust
pub struct ModemAudio {
    phase: AudioPhase,   // DialTone | Dialing(digit) | Ringing | Handshake | Data | Silent
    t: f64,              // wall-clock time accumulator
}
impl ModemAudio {
    pub fn advance(&mut self, samples: usize, sample_rate: u32, out: &mut [f32]);
}
```

`SerialModem` holds an `Option<ModemAudio>` (disabled when `--no-modem-sound` is passed). The `advance` method is called from the sound card's mix callback on the native platform.

DTMF tones are synthesized at runtime (two overlapping sine waves); the handshake chirp is a pre-recorded or procedurally generated 8-bit mono WAV embedded via `include_bytes!`.

### 7.3 CLI flag

```
--modem-sound / --no-modem-sound    Enable/disable modem audio (default: enabled)
```

This flag is a no-op on WASM (audio routing differs).

---

## Implementation Sequence Summary

| Phase | Deliverables | Tests |
|-------|-------------|-------|
| 1 | `SerialModem` stub, AT parser, CLI wiring, WASM config | `at_basic.asm` |
| 2 | Full AT command set, phonebook JSON, `ModemPhonebook` | `at_dial_reject.asm`, `at_hangup.asm` |
| 3 | TCP bridge thread, state machine, MSR lines | `tcp_echo.asm` |
| 4 | Echo server example, `modem_demo.asm`, README | Manual run |
| 5 | DOS software setup docs, Telix/Qmodem validation | Manual checklist |
| 6 | Docker Compose + tcpser + WWIV, phonebook entry | Manual BBS login |
| 7 | `ModemAudio`, DTMF synthesis, `--modem-sound` flag | Listening test |

---

## Key Files To Create / Modify

| File | Action |
|------|--------|
| `core/src/devices/modem.rs` | Create — `SerialModem` |
| `core/src/devices/modem_phonebook.rs` | Create — `ModemPhonebook` |
| `core/src/devices/mod.rs` | Add `pub mod modem; pub mod modem_phonebook;` |
| `core/src/tests/devices/modem.rs` | Create — all modem tests |
| `core/src/test_data/devices/modem/*.asm` | Create — test assembly |
| `native-common/src/cli.rs` | Add `--com1 modem`, `--modem-phonebook`, `--modem-sound` |
| `native-common/src/lib.rs` | Update `create_com_device()` for modem |
| `wasm/src/lib.rs` | Add modem fields to `WasmComputerConfig` |
| `examples/modem/modem_demo.asm` | Create |
| `examples/modem/echo_server/` | Create |
| `docker/bbs/docker-compose.yml` | Create |
| `docker/bbs/tcpser/Dockerfile` | Create |
| `docker/bbs/wwiv/Dockerfile` | Create |

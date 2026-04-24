# Modem Echo Demo

End-to-end demonstration of the oxide86 modem emulation. The demo program
initializes COM1, dials an echo server, sends a greeting, reads it back, then
hangs up cleanly.

## Prerequisites

- Rust toolchain (for the emulator and echo server)
- `nasm` (to assemble the demo, or use the precompiled `modem_demo.com`)

## Steps

### 1. Compile the demo (if needed)

```bash
nasm -f bin examples/modem/modem_demo.asm -o examples/modem/modem_demo.com
```

### 2. Start the echo server

```bash
cd examples/modem/echo_server
cargo run
```

The server listens on `0.0.0.0:2323` and echoes every byte back.

### 3. Create a phonebook

```bash
echo '{"0":"127.0.0.1:2323"}' > phonebook.json
```

### 4. Run the emulator

```bash
cargo run -p oxide86-cli -- \
  --com1 modem \
  --modem-phonebook phonebook.json \
  examples/modem/modem_demo.com
```

Expected output:

```
Hello, modem world!
Done.
```

## What the demo does

1. Initializes COM1 at 1200 baud via INT 14h
2. Sends `ATZ\r` and waits for `OK`
3. Sends `ATDT0\r` (phonebook key `0` → `127.0.0.1:2323`) and waits for `CONNECT`
4. Sends `Hello, modem world!\r\n` in data mode
5. Reads back the 22-byte echo and prints it to the screen
6. Sends `+++` to escape back to command mode (guard-time confirm: `OK`)
7. Sends `ATH\r` to hang up (response: `NO CARRIER`)
8. Prints `Done.` and exits with code 0

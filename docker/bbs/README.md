# WWIV BBS

A self-contained Docker Compose stack that runs a classic WWIV BBS reachable
via the oxide86 emulated modem.

```
DOS software → oxide86 modem (COM1) → TCP :2323 → WWIV
```

## Prerequisites

- Docker and Docker Compose
- oxide86 built (`cargo build -p oxide86-cli`)
- A DOS boot disk with terminal software (Telix, Procomm Plus, Qmodem, etc.)

## Start the stack

```bash
cd docker/bbs
docker compose up -d
```

The first start initializes the WWIV data volume; subsequent starts are fast.
Check logs with `docker compose logs -f`.

## Phonebook

Create `phonebook.json` in your working directory:

```json
{
  "1": "127.0.0.1:2323"
}
```

## Connect with oxide86

```bash
cargo run -p oxide86-cli -- \
  --com1 modem \
  --modem-phonebook phonebook.json \
  --boot --floppy-a msdos.img --floppy-b telix.img
```

Inside Telix (or any terminal software): dial `1`. The WWIV BBS login screen
appears after the connection is established.

## Stop

```bash
docker compose down
```

To also remove the BBS data volume (wipes all WWIV data):

```bash
docker compose down -v
```

## Notes

- WWIV BBS: https://www.wwivbbs.org/
- The WWIV image downloads the official Linux binary at build time; internet
  access is required when first building the image.
- The default sysop account is created by `init.sh` on first run. Log in as
  `Sysop` and change the password via the WWIV user management menu.

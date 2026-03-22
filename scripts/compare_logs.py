#!/usr/bin/env python3
"""
Compare two oxide86 execution logs by address only.
Handles timer-driven divergences by re-syncing after divergence blocks.
"""

import re
import sys
import argparse
from dataclasses import dataclass

# Matches both formats:
#   ] OP SSSS:OOOO HH HH ...   (older format with "OP" prefix)
#   ] SSSS:OOOO HH HH ...      (newer oxide86_core::cpu format, no "OP")
OP_RE = re.compile(r'\] (?:OP )?([0-9A-Fa-f]{4}:[0-9A-Fa-f]{4})')

# Bytes mode: captures the hex bytes between address and mnemonic.
# The mnemonic is separated from the bytes by 2+ spaces.
# Example: ] 36AC:50C0 83 C6 02             add si, 0x0002
BYTES_RE = re.compile(
    r'\] (?:OP )?[0-9A-Fa-f]{4}:[0-9A-Fa-f]{4} '
    r'((?:[0-9A-Fa-f]{2} )*[0-9A-Fa-f]{2})\s{2,}'
)


@dataclass
class Op:
    addr: str
    line_no: int


def parse_path_with_line(arg: str) -> tuple[str, int]:
    """Split 'file.log:452' into ('file.log', 452). Colon-less args return start=1."""
    if ':' in arg:
        # Handle Windows-style absolute paths like C:\foo by only splitting on the last colon
        # if what follows looks like a number
        parts = arg.rsplit(':', 1)
        if parts[1].isdigit():
            return parts[0], int(parts[1])
    return arg, 1


def parse_log(path: str, start_line: int = 1, mode: str = 'addr') -> list[Op]:
    regex = BYTES_RE if mode == 'bytes' else OP_RE
    ops = []
    with open(path, 'r', errors='replace') as f:
        for i, line in enumerate(f, 1):
            if i < start_line:
                continue
            m = regex.search(line)
            if m:
                key = m.group(1).upper()
                ops.append(Op(key, i))
    return ops


def find_resync(a_ops: list[Op], b_ops: list[Op], a_start: int, b_start: int,
                window: int = 200) -> tuple[int, int] | None:
    """Find the next position where both streams agree again."""
    a_slice = a_ops[a_start:a_start + window]
    b_slice = b_ops[b_start:b_start + window]
    # Build lookup of addr -> positions in b_slice
    b_index: dict[str, list[int]] = {}
    for bi, op in enumerate(b_slice):
        b_index.setdefault(op.addr, []).append(bi)
    # Find earliest pair (ai, bi) where both agree on 3 consecutive ops
    best = None
    for ai, op in enumerate(a_slice):
        for bi in b_index.get(op.addr, []):
            # Verify a few more ops match to avoid false positives
            match_len = 0
            for k in range(min(3, len(a_slice) - ai, len(b_slice) - bi)):
                oa = a_slice[ai + k]
                ob = b_slice[bi + k]
                if oa.addr == ob.addr:
                    match_len += 1
                else:
                    break
            if match_len >= 3:
                score = ai + bi
                if best is None or score < best[2]:
                    best = (ai, bi, score)
    if best:
        return (a_start + best[0], b_start + best[1])
    return None


def compare(a_ops: list[Op], b_ops: list[Op], context: int = 3, resync_window: int = 200):
    ai = 0
    bi = 0
    divergence_count = 0

    while ai < len(a_ops) and bi < len(b_ops):
        oa = a_ops[ai]
        ob = b_ops[bi]

        if oa.addr == ob.addr:
            ai += 1
            bi += 1
            continue

        # Divergence found — collect context before
        ctx_start_a = max(0, ai - context)
        ctx_start_b = max(0, bi - context)

        divergence_count += 1
        print(f"\n{'='*70}")
        print(f"DIVERGENCE #{divergence_count} at A[{ai}] (line {oa.line_no}) / B[{bi}] (line {ob.line_no})")
        print(f"{'='*70}")

        # Print context
        if ai > 0:
            print("  [context]")
            for k in range(ctx_start_a, ai):
                o = a_ops[k]
                print(f"    {o.addr}")

        # Try to resync
        sync = find_resync(a_ops, b_ops, ai, bi, resync_window)

        if sync is None:
            # No resync found — dump the rest
            print(f"\n  [A only — no resync found, showing remaining {len(a_ops) - ai} ops]")
            for o in a_ops[ai:ai + 20]:
                print(f"  < {o.addr}")
            print(f"\n  [B only — no resync found, showing remaining {len(b_ops) - bi} ops]")
            for o in b_ops[bi:bi + 20]:
                print(f"  > {o.addr}")
            ai = len(a_ops)
            bi = len(b_ops)
            break

        new_ai, new_bi = sync
        a_skipped = new_ai - ai
        b_skipped = new_bi - bi

        # Show diverging ops
        if a_skipped > 0:
            print(f"\n  [A only — {a_skipped} op(s)]")
            for o in a_ops[ai:new_ai]:
                print(f"  < {o.addr}")
        if b_skipped > 0:
            print(f"\n  [B only — {b_skipped} op(s)]")
            for o in b_ops[bi:new_bi]:
                print(f"  > {o.addr}")

        # Print context after resync
        print(f"\n  [resynced at A[{new_ai}] / B[{new_bi}]]")
        for k in range(new_ai, min(new_ai + context, len(a_ops))):
            o = a_ops[k]
            print(f"    {o.addr}")

        ai = new_ai
        bi = new_bi

    # Report if one stream ended before the other
    remaining_a = len(a_ops) - ai
    remaining_b = len(b_ops) - bi
    if remaining_a > 0 or remaining_b > 0:
        print(f"\n{'='*70}")
        print(f"END OF COMPARISON")
        if remaining_a > 0:
            print(f"  A has {remaining_a} more ops after B ended (last: {a_ops[-1].addr})")
        if remaining_b > 0:
            print(f"  B has {remaining_b} more ops after A ended (last: {b_ops[-1].addr})")

    print(f"\n{'='*70}")
    print(f"Total divergences: {divergence_count}")
    print(f"A ops: {len(a_ops)}, B ops: {len(b_ops)}")


def main():
    parser = argparse.ArgumentParser(
        description='Compare two oxide86 execution logs by address.')
    parser.add_argument('log_a', nargs='?', help='First (reference) log')
    parser.add_argument('log_b', nargs='?', help='Second log to compare')
    parser.add_argument('-c', '--context', type=int, default=3,
                        help='Context lines around divergences (default: 3)')
    parser.add_argument('-w', '--window', type=int, default=200,
                        help='Look-ahead window for resync search (default: 200)')
    parser.add_argument('--max-divergences', type=int, default=0,
                        help='Stop after N divergences (0 = unlimited)')
    parser.add_argument('-m', '--mode', choices=['addr', 'bytes'], default='addr',
                        help='Comparison mode: addr (default) or bytes')
    args = parser.parse_args()

    if not args.log_a or not args.log_b:
        parser.print_help()
        sys.exit(1)

    log_a_path, log_a_start = parse_path_with_line(args.log_a)
    log_b_path, log_b_start = parse_path_with_line(args.log_b)

    print(f"Mode: {args.mode}")
    print(f"Loading {log_a_path} (from line {log_a_start})...", end=' ', flush=True)
    a_ops = parse_log(log_a_path, log_a_start, args.mode)
    print(f"{len(a_ops)} ops")

    print(f"Loading {log_b_path} (from line {log_b_start})...", end=' ', flush=True)
    b_ops = parse_log(log_b_path, log_b_start, args.mode)
    print(f"{len(b_ops)} ops")

    if not a_ops:
        print("ERROR: no OP lines found in", args.log_a, file=sys.stderr)
        sys.exit(1)
    if not b_ops:
        print("ERROR: no OP lines found in", args.log_b, file=sys.stderr)
        sys.exit(1)

    compare(a_ops, b_ops, context=args.context, resync_window=args.window)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""
bda_logs_analyze.py — BDA timer tick rate analyzer for oxide86.log

Counts emulated instructions between timer ticks and reports whether the
emulated clock is running at the expected ~18.2 Hz.

Detection modes (in order of preference):

  --tick-addr  SEG:OFF   Count instructions between occurrences of this address.
                         Use when you know the tick-handler entry point.
                         Example: --tick-addr 1306:0300  (CheckIt INT 08h hook)

  --tick-disasm REGEX    Count instructions between lines whose disasm matches REGEX.
                         Example: --tick-disasm "add \\[0x0c7a\\]"

  auto (default)         Watch the BDA timer word at 0x046C for value changes.
                         Works when the log contains memory-read debug annotations
                         such as "[0x046c]=NNNN" on instruction lines.

The BDA timer lives at physical 0x0000:0x046C and is incremented by the BIOS
INT 08h handler at ~18.2 Hz (every 54,925 µs on real hardware). At 8 MHz with
1 CPI that is roughly 439,560 cycles = ~439,560 instructions between ticks.

Usage:
  python scripts/bda_logs_analyze.py [oxide86.log]
  python scripts/bda_logs_analyze.py oxide86.log --tick-addr 1306:0300
  python scripts/bda_logs_analyze.py oxide86.log --tick-disasm "add \\[0x0c7a\\]"
  python scripts/bda_logs_analyze.py oxide86.log --cpu-mhz 8 --cpi 1.5
  python scripts/bda_logs_analyze.py oxide86.log --threshold 218 --bda-start 37
"""

import argparse
import re
import sys
from statistics import mean, median, stdev

PIT_OSCILLATOR_HZ = 1_193_182.0   # 8254 PIT base frequency


# --------------------------------------------------------------------------- #
# Phase detection
# --------------------------------------------------------------------------- #

def _win_median(values, lo, hi):
    """Median of values[lo:hi] (hi exclusive)."""
    return median(values[lo:hi])


def find_phases(gaps, cpu_hz, cpi, change_pct=0.04, window=5, min_phase=5):
    """
    Segment inter-tick gap counts into stable/variable rate phases.

    Uses a back-vs-forward sliding window: at each position a new phase begins
    when the forward-window median differs from the backward-window median by
    more than change_pct, provided at least min_phase gaps have elapsed since
    the last boundary.

    Returns a list of dicts, one per phase:
      start      0-based gap index (inclusive)
      end        0-based gap index (inclusive)
      count      number of gaps
      mean_hz    mean implied Hz for this phase
      stdev_hz   stdev of implied Hz
      min_hz     min implied Hz
      max_hz     max implied Hz
      label      'stable' or 'variable'
      pit_div    PIT divisor implied by mean_hz (PIT_OSC / mean_hz)
    """
    n = len(gaps)
    if n == 0:
        return []

    hz = [cpu_hz / (g * cpi) for g in gaps]

    boundaries = [0]
    last_bp = 0

    for i in range(window, n - window + 1):
        back = _win_median(hz, max(last_bp, i - window), i)
        fwd  = _win_median(hz, i, min(n, i + window))
        if back > 0 and abs(fwd - back) / back > change_pct:
            if i - last_bp >= min_phase:
                boundaries.append(i)
                last_bp = i

    boundaries.append(n)

    phases = []
    for j in range(len(boundaries) - 1):
        s = boundaries[j]
        e = boundaries[j + 1] - 1
        ph_hz = hz[s:e + 1]
        m  = mean(ph_hz)
        sd = stdev(ph_hz) if len(ph_hz) >= 2 else 0.0
        cv = sd / m if m > 0 else 0.0
        phases.append({
            'start':    s,
            'end':      e,
            'count':    e - s + 1,
            'mean_hz':  m,
            'stdev_hz': sd,
            'min_hz':   min(ph_hz),
            'max_hz':   max(ph_hz),
            'label':    'stable' if cv < 0.05 else 'variable',
            'pit_div':  PIT_OSCILLATOR_HZ / m if m > 0 else 0,
        })
    return phases


def print_phases(phases, cpi):
    """Print the compact rate-phase summary table."""
    if not phases:
        return

    print('Rate phases:')
    for p in phases:
        # Gap indices are 0-based; tick display is 1-based and offset by 1
        # gap[i] = instrs between tick_event[i] and tick_event[i+1]
        # so gap[0] → "tick 1→2", displayed range is ticks (s+1)–(e+2)
        t_lo = p['start'] + 1
        t_hi = p['end']   + 2
        rng  = f"ticks {t_lo:3d}–{t_hi:3d}  ({p['count']:3d} gaps)"

        if p['label'] == 'stable':
            hz_str  = f"{p['mean_hz']:6.1f} Hz ±{p['stdev_hz']:.1f}"
            div_str = f"  PIT div≈{p['pit_div']:,.0f}"
            print(f"  {rng}:  stable    {hz_str}{div_str}")
        else:
            hz_str = f"{p['min_hz']:.0f}–{p['max_hz']:.0f} Hz"
            print(f"  {rng}:  variable  ({hz_str})")
    print()

# --------------------------------------------------------------------------- #
# Regexes
# --------------------------------------------------------------------------- #

# Matches CPU instruction log lines:
#   [HH:MM:SS.mmm INFO  oxide86_core::cpu] SEG:OFF  BYTES  DISASM   [debug...]
LOG_CPU_RE = re.compile(
    r'oxide86_core::cpu\] '
    r'([0-9A-Fa-f]{4}:[0-9A-Fa-f]{4}) '   # group 1: SEG:OFF
    r'(?:[0-9A-Fa-f]{2} )*[0-9A-Fa-f]{2}' # bytes (not captured)
    r'\s+'
    r'(.*)'                                 # group 2: rest of line (disasm + debug)
)

# Matches BDA timer low-word value in memory-access debug annotations:
#   [0x046c]=NNNN  or  [0x046C]=NNNN
BDA_VALUE_RE = re.compile(r'\[0x046[cC]\]=([0-9a-fA-F]+)')


# --------------------------------------------------------------------------- #
# Helpers
# --------------------------------------------------------------------------- #

def normalise_addr(addr: str) -> str:
    seg, off = addr.upper().split(':')
    return f"{int(seg, 16):04X}:{int(off, 16):04X}"


def parse_args():
    p = argparse.ArgumentParser(
        description='Analyze BDA timer tick rate from oxide86.log',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument('log', nargs='?', default='oxide86.log',
                   help='Path to oxide86.log (default: oxide86.log)')
    p.add_argument('--tick-addr', metavar='SEG:OFF',
                   help='Count ticks at this instruction address (e.g. 1306:0300)')
    p.add_argument('--tick-disasm', metavar='REGEX',
                   help='Count ticks when disasm matches this regex')
    p.add_argument('--cpu-mhz', type=float, default=8.0,
                   help='Emulated CPU frequency in MHz (default: 8.0)')
    p.add_argument('--cpi', type=float, default=1.0,
                   help='Assumed cycles per instruction (default: 1.0)')
    p.add_argument('--threshold', type=int, default=218,
                   help='BDA tick threshold the program is waiting for (default: 218)')
    p.add_argument('--bda-start', type=int, default=None,
                   help='BDA tick value at test start (default: auto-detect from first tick)')
    p.add_argument('--verbose', '-v', action='store_true',
                   help='Print per-tick detail lines')
    p.add_argument('--top-candidates', action='store_true',
                   help='Print top repeated addresses (useful for finding tick address)')
    return p.parse_args()


# --------------------------------------------------------------------------- #
# Main analysis
# --------------------------------------------------------------------------- #

def analyse(args):
    log_path = args.log
    cpu_hz = args.cpu_mhz * 1_000_000
    cpi = args.cpi

    tick_addr = normalise_addr(args.tick_addr) if args.tick_addr else None
    tick_disasm_re = re.compile(args.tick_disasm, re.IGNORECASE) if args.tick_disasm else None

    # Mode label
    if tick_addr:
        mode = f'tick-addr={tick_addr}'
    elif tick_disasm_re:
        mode = f'tick-disasm=/{args.tick_disasm}/'
    else:
        mode = 'auto (BDA 0x046C value change)'

    instr_count = 0          # total instructions seen
    tick_events = []         # list of (instr_count_at_tick, bda_value_or_None)
    last_bda_value = None    # for auto-detect mode
    addr_freq = {}           # for --top-candidates

    try:
        f = open(log_path, 'r', errors='replace')
    except FileNotFoundError:
        sys.exit(f'Error: log file not found: {log_path}')

    with f:
        for line in f:
            m = LOG_CPU_RE.search(line)
            if not m:
                continue

            addr = m.group(1).upper()
            rest = m.group(2)   # disasm + optional debug info

            instr_count += 1

            if args.top_candidates:
                addr_freq[addr] = addr_freq.get(addr, 0) + 1

            # --- Detect tick event ---
            bda_val = None

            if tick_addr:
                if addr == tick_addr:
                    tick_events.append((instr_count, bda_val))

            elif tick_disasm_re:
                # disasm is everything before 2+ spaces of debug info
                disasm = re.split(r'\s{2,}', rest)[0].strip()
                if tick_disasm_re.search(disasm):
                    tick_events.append((instr_count, bda_val))

            else:
                # Auto: detect BDA 0x046C value changes
                bm = BDA_VALUE_RE.search(line)
                if bm:
                    bda_val = int(bm.group(1), 16)
                    if last_bda_value is not None and bda_val != last_bda_value:
                        # Value changed — a tick occurred between the previous read and now
                        tick_events.append((instr_count, bda_val))
                    last_bda_value = bda_val

    if args.top_candidates:
        print('Top 20 most-executed addresses (candidates for --tick-addr):')
        top = sorted(addr_freq.items(), key=lambda x: -x[1])[:20]
        for a, c in top:
            print(f'  {a}  {c:8,}')
        print()

    if not tick_events:
        print(f'No tick events detected (mode: {mode}).')
        print()
        print('Suggestions:')
        print('  • Run with --top-candidates to see frequent addresses')
        print('  • Use --tick-addr 1306:0300 if tracing CheckIt with its INT 08h hook')
        print('  • Use --tick-disasm "add \\[0x0c7a\\]" to match CheckIt\'s tick counter increment')
        print('  • Ensure log was captured with DEBUG/INFO level cpu logging enabled')
        return

    # --- Compute inter-tick gaps ---
    gaps = []
    bda_values_at_tick = []
    for i, (cnt, bval) in enumerate(tick_events):
        if i == 0:
            prev = cnt
            if bval is not None:
                bda_values_at_tick.append(bval)
            continue
        gaps.append(cnt - tick_events[i - 1][0])
        if bval is not None:
            bda_values_at_tick.append(bval)

    n_ticks = len(tick_events)
    n_gaps = len(gaps)

    # --- Hz derivation ---
    # instructions_per_tick * cpi = cycles_per_tick
    # Hz = cpu_hz / cycles_per_tick
    avg_gap = mean(gaps) if gaps else float('nan')
    implied_hz = cpu_hz / (avg_gap * cpi) if avg_gap > 0 else float('nan')
    expected_gap = cpu_hz / (18.2065 * cpi)   # expected at real 18.2 Hz

    # --- BDA elapsed ticks since test start ---
    if bda_values_at_tick:
        bda_start = args.bda_start if args.bda_start is not None else bda_values_at_tick[0]
        bda_end = bda_values_at_tick[-1]
        bda_elapsed = (bda_end - bda_start) & 0xFFFF   # 16-bit wrap
    else:
        bda_start = args.bda_start
        bda_elapsed = None

    # --- Output ---
    print(f'BDA Timer Tick Rate Analysis')
    print(f'  Log:           {log_path}')
    print(f'  Mode:          {mode}')
    print(f'  CPU:           {args.cpu_mhz:.1f} MHz, CPI={cpi:.2f}')
    print(f'  Total instrs:  {instr_count:,}')
    print(f'  Tick events:   {n_ticks}  (gaps computed: {n_gaps})')
    print()

    if args.verbose and gaps:
        print('Per-tick instruction counts:')
        for i, g in enumerate(gaps, 1):
            hz = cpu_hz / (g * cpi)
            flag = ''
            if hz < 15:
                flag = '  ← slow'
            elif hz > 25:
                flag = '  ← fast'
            bda_note = ''
            if i < len(bda_values_at_tick):
                bda_note = f'  BDA={bda_values_at_tick[i]:04X}h'
            print(f'  tick {i:4d}: {g:10,} instrs  →  {hz:6.2f} Hz{bda_note}{flag}')
        print()

    if gaps:
        print('Inter-tick instruction counts:')
        print(f'  avg:    {avg_gap:>12,.1f} instrs/tick')
        print(f'  median: {median(gaps):>12,.1f} instrs/tick')
        print(f'  min:    {min(gaps):>12,} instrs/tick')
        print(f'  max:    {max(gaps):>12,} instrs/tick')
        if n_gaps >= 2:
            print(f'  stdev:  {stdev(gaps):>12,.1f} instrs/tick')
        print()
        print(f'Implied tick rate:   {implied_hz:6.2f} Hz  (at {args.cpu_mhz:.0f} MHz, CPI={cpi:.2f})')
        print(f'Target tick rate:    18.21 Hz')
        print(f'Expected gap:        {expected_gap:>12,.0f} instrs/tick  (at 18.2 Hz)')
        ratio = implied_hz / 18.2065
        print(f'Clock ratio:         {ratio:.3f}x  ', end='')
        if 0.95 <= ratio <= 1.05:
            print('✓ within 5% of real hardware')
        elif ratio < 1:
            print(f'← ticking {1/ratio:.1f}x too slowly')
        else:
            print(f'← ticking {ratio:.1f}x too fast')
        print()

        phases = find_phases(gaps, cpu_hz, cpi)
        print_phases(phases, cpi)

    if bda_values_at_tick and bda_elapsed is not None:
        print(f'BDA counter progression:')
        print(f'  Start value:    {bda_start} (0x{bda_start:04X})')
        print(f'  End value:      {bda_end} (0x{bda_end:04X})')
        print(f'  Elapsed ticks:  {bda_elapsed}')
        threshold = args.threshold
        remaining = threshold - bda_elapsed
        print(f'  Threshold:      {threshold}')
        if remaining > 0:
            pct = 100 * bda_elapsed / threshold
            print(f'  Remaining:      {remaining} ticks  ({pct:.1f}% complete)')
            if avg_gap > 0 and n_ticks > 0:
                # Outer loop iterations ≈ instrs between ticks / instrs per outer-loop iter
                # Approximate: remaining ticks × avg_gap instructions each
                remaining_instrs = remaining * avg_gap
                print(f'  ~{remaining_instrs:,.0f} more instructions to reach threshold')
                # Ticks per second
                tick_wall_s = avg_gap * cpi / cpu_hz
                remaining_wall_s = remaining * tick_wall_s
                print(f'  ~{remaining_wall_s:.1f}s of emulated CPU time remaining')
        else:
            print(f'  ✓ Threshold already reached (surplus: {-remaining} ticks)')
        print()

    # Sanity check for --tick-addr: warn if gap is suspiciously large
    if tick_addr and gaps and max(gaps) > 10_000_000:
        print('Warning: some inter-tick gaps exceed 10M instructions.')
        print('  This may indicate missed tick events or a very slow emulated clock.')
        print()


if __name__ == '__main__':
    analyse(parse_args())

#!/usr/bin/env python3
"""
Parse oxide86.log and output unique instructions with execution counts.

Output format:
  SEG:OFF  BYTES                DISASM             COUNT

Function entry points (call targets) are labelled with func_SSSS_OOOO:.
"""

import json
import re
import sys
from collections import defaultdict

# Match lines like:
#   [19:59:15.695 INFO  oxide86_core::cpu] 31A9:4FA7 AB                   stosw
#   [18:09:41.612 INFO  oxide86_core::cpu] 31A9:43AE 26 8B 5C 02  mov bx, [si+0x02]  BX=F5AD [0xf3a8]=f5ad @48A2:F3A8(57DC8)
LOG_RE = re.compile(
    r'\] ([0-9A-Fa-f]{4}:[0-9A-Fa-f]{4}) ((?:[0-9A-Fa-f]{2} )*[0-9A-Fa-f]{2})\s+(.*?)(?:\s{2,}(.*))?$'
)

# near call: call 0xABCD
CALL_NEAR_RE = re.compile(r'^call\s+(0x[0-9a-fA-F]+)$')
# far call: call far 0xSEG, 0xOFF
CALL_FAR_RE = re.compile(r'^call\s+far\s+(0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+)$')
# software interrupt: int 0xNN
INT_RE = re.compile(r'^int\s+(0x[0-9a-fA-F]+)$')
# near jump: jmp/jmp short/jmp near/jcc/loop/jcxz 0xABCD
JMP_NEAR_RE = re.compile(r'^(jmp(?:\s+(?:short|near))?|j[a-z]+|loop[a-z]*|jcxz)\s+(0x[0-9a-fA-F]+)$')
# far jump: jmp far 0xSEG, 0xOFF
JMP_FAR_RE = re.compile(r'^jmp\s+far\s+(0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+)$')


def parse_log(path):
    counts = defaultdict(int)
    info = {}           # (addr, bytes) -> disasm
    values = {}         # (addr, bytes) -> set of values strings (None = varies)
    call_targets = set()
    jump_targets = set()
    int_handlers = {}   # addr -> set of int numbers (handler entry points)

    pending_int = None  # (int_number, seg_of_int_instr) or None

    with open(path, 'r', errors='replace') as f:
        for line in f:
            m = LOG_RE.search(line)
            if not m:
                continue
            addr = m.group(1).upper()
            bytecode = m.group(2).strip()
            disasm = m.group(3).strip()
            val = (m.group(4) or '').strip()
            key = (addr, bytecode)
            counts[key] += 1
            if key not in info:
                info[key] = disasm
            if key not in values:
                values[key] = val
            elif values[key] != val:
                values[key] = None  # differs across executions

            # If the previous instruction was an `int NN`, the next instruction
            # in the log is the handler entry — but only if it's in a different
            # segment.  If the handler code isn't logged (e.g. BIOS ROM), the
            # next logged instruction is back in the caller's segment and should
            # NOT be labelled as the handler.
            if pending_int is not None:
                int_num, int_seg = pending_int
                if addr[:4] != int_seg:
                    int_handlers.setdefault(addr, set()).add(int_num)
                pending_int = None

            seg = addr[:4]

            near = CALL_NEAR_RE.match(disasm)
            if near:
                off = int(near.group(1), 16)
                call_targets.add(f"{seg}:{off:04X}")
                continue

            far = CALL_FAR_RE.match(disasm)
            if far:
                tseg = int(far.group(1), 16)
                toff = int(far.group(2), 16)
                call_targets.add(f"{tseg:04X}:{toff:04X}")
                continue

            int_m = INT_RE.match(disasm)
            if int_m:
                pending_int = (int(int_m.group(1), 16), seg)
                continue

            jnear = JMP_NEAR_RE.match(disasm)
            if jnear:
                off = int(jnear.group(2), 16)
                jump_targets.add(f"{seg}:{off:04X}")
                continue

            jfar = JMP_FAR_RE.match(disasm)
            if jfar:
                tseg = int(jfar.group(1), 16)
                toff = int(jfar.group(2), 16)
                jump_targets.add(f"{tseg:04X}:{toff:04X}")

    return counts, info, values, call_targets, jump_targets, int_handlers


def load_config(path):
    """Load optional JSON config. Returns (functions_dict, labels_dict, line_comments_dict, retf_targets_dict)."""
    try:
        with open(path, 'r') as f:
            data = json.load(f)
    except FileNotFoundError:
        return {}, {}, {}, {}
    # Normalise keys to uppercase
    functions = {k.upper(): v for k, v in data.get('functions', {}).items()}
    labels = {k.upper(): v for k, v in data.get('labels', {}).items()}
    line_comments = {k.upper(): v for k, v in data.get('lineComments', {}).items()}
    retf_targets = {k.upper(): v for k, v in data.get('retf_targets', {}).items()}
    return functions, labels, line_comments, retf_targets


def _wrap_comment(text, width=80):
    """Wrap text into '; ...' comment lines, breaking on spaces at `width`."""
    prefix = '; '
    lines = []
    for paragraph in text.splitlines():
        words = paragraph.split()
        if not words:
            lines.append(prefix)
            continue
        current = prefix
        for word in words:
            candidate = current + ('' if current == prefix else ' ') + word
            if len(candidate) > width and current != prefix:
                lines.append(current)
                current = prefix + word
            else:
                current = candidate
        lines.append(current)
    return lines


def main():
    log_path = sys.argv[1] if len(sys.argv) > 1 else 'oxide86.log'
    config_path = sys.argv[2] if len(sys.argv) > 2 else None

    counts, info, values, call_targets, jump_targets, int_handlers = parse_log(log_path)
    functions, labels, line_comments, retf_targets = load_config(config_path) if config_path else ({}, {}, {}, {})

    # Sort by segment, then offset, then bytecode
    keys = sorted(counts.keys(), key=lambda k: (k[0][0:4], int(k[0][5:], 16), k[1]))

    print("; Generated by scripts/logs_to_asm.py")
    print("; Additional information can be found in scripts/logs_to_asm.md")
    print(f"; Log: {log_path}")
    if config_path:
        print(f"; Config: {config_path}")
    print("")

    prev_seg = None
    prev_end_off = None

    for key in keys:
        addr, bytecode = key
        disasm = info[key]
        count = counts[key]

        seg, off_str = addr.split(':')
        cur_off = int(off_str, 16)
        byte_len = len(bytecode.split())

        # Detect gaps within the same segment
        if prev_seg == seg and prev_end_off is not None and cur_off > prev_end_off:
            gap_bytes = cur_off - prev_end_off
            print(f"   ; gap {seg}:{prev_end_off:04X} - {seg}:{cur_off:04X} ({gap_bytes} bytes)")

        if prev_seg != seg:
            prev_seg = seg
            prev_end_off = cur_off + byte_len
        else:
            prev_end_off = max(prev_end_off, cur_off + byte_len)

        if addr in int_handlers:
            for n in sorted(int_handlers[addr]):
                print(f"\nint_{n:02x}h:")

        if addr in call_targets or addr in retf_targets:
            func = functions.get(addr) or retf_targets.get(addr, {})
            print()
            if func.get('comment'):
                for line in _wrap_comment(func['comment']):
                    print(line)
            if func.get('label'):
                print(f"{func['label']}:   ; {addr}")
            else:
                seg, off = addr.split(':')
                print(f"func_{seg}_{off}:")
        elif addr in jump_targets:
            lbl = labels.get(addr, {})
            if lbl.get('comment'):
                print()
                for line in _wrap_comment(lbl['comment']):
                    print(line)
            if lbl.get('label'):
                print(f"{lbl['label']}:   ; {addr}")
            else:
                print(f"lbl_{seg}_{off_str}:")

        # Resolve call/jump target label for annotating the instruction
        call_label = ''
        near = CALL_NEAR_RE.match(disasm)
        if near:
            seg = addr[:4]
            off = int(near.group(1), 16)
            target = f"{seg}:{off:04X}"
            func = functions.get(target, {})
            if func.get('label'):
                call_label = func['label']
            else:
                call_label = f"func_{seg}_{off:04X}"
        else:
            far = CALL_FAR_RE.match(disasm)
            if far:
                tseg = int(far.group(1), 16)
                toff = int(far.group(2), 16)
                target = f"{tseg:04X}:{toff:04X}"
                func = functions.get(target, {})
                if func.get('label'):
                    call_label = func['label']
                else:
                    call_label = f"func_{tseg:04X}_{toff:04X}"
            else:
                jnear = JMP_NEAR_RE.match(disasm)
                if jnear:
                    joff = int(jnear.group(2), 16)
                    jtarget = f"{seg}:{joff:04X}"
                    if jtarget in call_targets or jtarget in retf_targets:
                        func = functions.get(jtarget) or retf_targets.get(jtarget, {})
                        call_label = func.get('label') or f"func_{seg}_{joff:04X}"
                    else:
                        lbl = labels.get(jtarget, {})
                        call_label = lbl.get('label') or f"lbl_{seg}_{joff:04X}"
                else:
                    jfar = JMP_FAR_RE.match(disasm)
                    if jfar:
                        jtseg = int(jfar.group(1), 16)
                        jtoff = int(jfar.group(2), 16)
                        jtarget = f"{jtseg:04X}:{jtoff:04X}"
                        if jtarget in call_targets or jtarget in retf_targets:
                            func = functions.get(jtarget) or retf_targets.get(jtarget, {})
                            call_label = func.get('label') or f"func_{jtseg:04X}_{jtoff:04X}"
                        else:
                            lbl = labels.get(jtarget, {})
                            call_label = lbl.get('label') or f"lbl_{jtseg:04X}_{jtoff:04X}"

        line_key = f"{addr} {bytecode}"
        line_comment = line_comments.get(line_key, '')
        if line_comment:
            for cline in _wrap_comment(line_comment, width=80):
                print(f"   {cline}")
        comment_col = f"{call_label}  " if call_label else ''
        val = values.get(key)
        val_col = f"  [{val}]" if val else ''
        print(f"   {disasm:<24}; {count:4} -- {addr} {bytecode:<19}{comment_col} {val_col}")


if __name__ == '__main__':
    main()

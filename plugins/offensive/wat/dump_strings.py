#!/usr/bin/env python3
"""Dump string data from the compiled WASM file's data sections."""
import struct
import sys

if len(sys.argv) != 2:
    print("usage: dump_strings.py <file.wasm>")
    sys.exit(1)

path = sys.argv[1]
with open(path, "rb") as f:
    data = f.read()

assert data[:4] == b"\0asm"
i = 8
section_id_names = {0: "custom", 1: "type", 2: "import", 3: "function", 4: "table",
                    5: "memory", 6: "global", 7: "export", 8: "start", 9: "element",
                    10: "code", 11: "data", 12: "data_count"}

while i < len(data):
    sid = data[i]; i += 1
    size, consumed = 0, 0
    shift = 0
    while True:
        b = data[i + consumed]
        consumed += 1
        size |= (b & 0x7F) << shift
        if not (b & 0x80):
            break
        shift += 7
    sec_end = i + consumed + size
    name = section_id_names.get(sid, f"id={sid}")
    print(f"--- Section {sid} ({name}) at offset {i-1}, size {size} ---")
    if sid == 0:  # custom
        # parse name
        nl, nc = 0, 0
        while True:
            b = data[i + nc]
            nc += 1
            nl |= (b & 0x7F) << (nc * 7 - 7)
            if not (b & 0x80):
                break
        custom_name = data[i + nc:i + nc + nl].decode("utf-8", errors="replace")
        print(f"  custom name: {custom_name!r}")
        # skip
    elif sid == 11:  # data
        # parse data segment count
        cnt, cc = 0, 0
        shift = 0
        while True:
            b = data[i + cc]
            cc += 1
            cnt |= (b & 0x7F) << shift
            if not (b & 0x80):
                break
            shift += 7
        print(f"  data segments: {cnt}")
        j = i + cc
        for k in range(cnt):
            # parse flags
            flags = data[j]; j += 1
            if flags & 0x01:
                # i32.const offset
                val, vc = 0, 0
                shift = 0
                while True:
                    b = data[j + vc]
                    vc += 1
                    val |= (b & 0x7F) << shift
                    if not (b & 0x80):
                        break
                    shift += 7
                j += vc
                offset = val
            else:
                offset = 0
            # length
            length, lc = 0, 0
            shift = 0
            while True:
                b = data[j + lc]
                lc += 1
                length |= (b & 0x7F) << shift
                if not (b & 0x80):
                    break
                shift += 7
            j += lc
            payload = data[j:j + length]
            j += length
            try:
                decoded = payload.decode("utf-8")
            except UnicodeDecodeError:
                decoded = repr(payload)
            print(f"  segment[{k}] @ {offset} len={length}: {decoded!r}")
    i = sec_end


#!/usr/bin/env python3
"""Properly parse WASM data sections and dump memory contents at known offsets."""
import sys

if len(sys.argv) < 2:
    print("usage: dump_memory.py <file.wasm> [offset1] [offset2] ...")
    sys.exit(1)

path = sys.argv[1]
with open(path, "rb") as f:
    data = f.read()

# Find data section (id=11) by walking sections
i = 8
data_section = None
while i < len(data):
    sid = data[i]
    # parse section size
    j = i + 1
    size = 0
    shift = 0
    while True:
        b = data[j]; j += 1
        size |= (b & 0x7F) << shift
        if not (b & 0x80): break
        shift += 7
    if sid == 11:
        data_section = (j, size)
        break
    i = j + size

if not data_section:
    print("No data section found!")
    sys.exit(1)

j, sec_size = data_section
sec_end = j + sec_size
print(f"Data section at byte offset {j}, size {sec_size}")

# parse segment count
seg_count = 0
shift = 0
while True:
    b = data[j]; j += 1
    seg_count |= (b & 0x7F) << shift
    if not (b & 0x80): break
    shift += 7
print(f"Segment count: {seg_count}")

# Combine all data into a single memory dict
mem = {}
segments = []
for k in range(seg_count):
    flags = data[j]; j += 1
    # Active with i32.const: flags = 0
    # Read 0x41 (i32.const), LEB128, 0x0B (end)
    assert flags == 0, f"unexpected flags {flags:#x}"
    assert data[j] == 0x41, f"expected i32.const, got {data[j]:#x}"
    j += 1
    val = 0; shift = 0
    while True:
        b = data[j]; j += 1
        val |= (b & 0x7F) << shift
        if not (b & 0x80): break
        shift += 7
    assert data[j] == 0x0B, f"expected end, got {data[j]:#x}"
    j += 1
    length = 0; shift = 0
    while True:
        b = data[j]; j += 1
        length |= (b & 0x7F) << shift
        if not (b & 0x80): break
        shift += 7
    payload = data[j:j+length]
    j += length
    segments.append((val, length, payload))
    for p, b in enumerate(payload):
        mem[val + p] = b

# Print every segment
print(f"\n=== {len(segments)} segments ===")
for off, ln, p in segments:
    try:
        s = p.decode("utf-8")
    except UnicodeDecodeError:
        s = repr(p)
    print(f"  @{off} len={ln}: {s!r}")

# Print memory at requested offsets
if len(sys.argv) > 2:
    print("\n=== Memory at requested offsets ===")
    for arg in sys.argv[2:]:
        off = int(arg)
        # find the segment that contains this offset
        containing = [s for s in segments if s[0] <= off < s[0] + s[1]]
        if containing:
            seg_off, seg_len, payload = containing[0]
            rel = off - seg_off
            # show 80 bytes
            chunk = payload[rel:rel+80]
            print(f"  @{off} (segment @{seg_off}, rel={rel}): {chunk!r}")
        else:
            # show raw memory
            chunk = bytes(mem.get(off + k, 0xFF) for k in range(80))
            print(f"  @{off} (no segment): {chunk!r}")


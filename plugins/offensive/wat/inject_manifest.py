#!/usr/bin/env python3
"""
Inject a 'slpx_manifest' custom section into a WASM file.
Format:
  - Section ID = 0 (custom)
  - Section size (LEB128 u32)
  - Name length (LEB128 u32)
  - Name bytes ("slpx_manifest")
  - JSON payload bytes
"""
import json
import struct
import sys


def leb128_u32(n: int) -> bytes:
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n == 0:
            out.append(b)
            return bytes(out)
        out.append(b | 0x80)


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: inject_manifest.py <input.wasm> <manifest.json> <output.wasm>",
              file=sys.stderr)
        return 1

    in_path, manifest_path, out_path = sys.argv[1], sys.argv[2], sys.argv[3]

    with open(in_path, "rb") as f:
        wasm = bytearray(f.read())
    with open(manifest_path, "rb") as f:
        manifest_bytes = f.read()

    name = b"slpx_manifest"
    name_len = leb128_u32(len(name))
    payload = name_len + name + manifest_bytes
    section = bytes([0x00]) + leb128_u32(len(payload)) + payload

    if wasm[:4] != b"\0asm":
        print("Not a WASM file", file=sys.stderr)
        return 1
    if wasm[4:8] != struct.pack("<I", 1):
        print("Unexpected WASM version", file=sys.stderr)
        return 1

    out = bytearray()
    out += wasm[:8]  # magic + version
    i = 8
    inserted = False
    while i < len(wasm):
        sid = wasm[i]
        i += 1
        size, consumed = 0, 0
        shift = 0
        while True:
            b = wasm[i + consumed]
            consumed += 1
            size |= (b & 0x7F) << shift
            if not (b & 0x80):
                break
            shift += 7
        sec_end = i + consumed + size
        if not inserted and sid >= 12:
            out += section
            inserted = True
        out += wasm[i - 1:sec_end]
        i = sec_end
    if not inserted:
        out += section

    with open(out_path, "wb") as f:
        f.write(out)

    print(f"Injected {len(manifest_bytes)} byte manifest into {in_path} -> {out_path} "
          f"({len(wasm)} -> {len(out)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())


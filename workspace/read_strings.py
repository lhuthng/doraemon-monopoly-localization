#!/usr/bin/env python3
"""Read workspace/strings.dat: parse the GameOne archive, decompress each
LZW-compressed leaf, and print each decoded byte as an ASCII character."""

import struct
import sys

SIGNATURE = b"\0\0GameOne Systems Limited\nWritten by Samme NG\0"


def u32(data, offset):
    return struct.unpack_from("<I", data, offset)[0]


def is_container(data, offset):
    return data[offset : offset + len(SIGNATURE)] == SIGNATURE


def collect_nodes(data, offset, path, nodes):
    count = u32(data, offset + 0x42)
    table = offset + 0x66
    for index in range(count):
        child = offset + u32(data, table + index * 4)
        child_path = path + [index]
        if is_container(data, child):
            collect_nodes(data, child, child_path, nodes)
        else:
            nodes.append((child_path, child))


def archive_nodes(data):
    nodes = []
    collect_nodes(data, 0, [], nodes)
    return nodes


class CodeReader:
    def __init__(self, data):
        self.data = data
        self.pos = 0
        self.bits = 0
        self.bit_count = 0

    def read(self):
        while self.bit_count < 14:
            self.bits = (self.bits << 8) | self.data[self.pos]
            self.pos += 1
            self.bit_count += 8
        self.bit_count -= 14
        code = (self.bits >> self.bit_count) & 0x3FFF
        self.bits &= (1 << self.bit_count) - 1
        return code


def decompress(payload):
    if len(payload) < 5:
        raise ValueError("payload too small")
    expected = u32(payload, 0)
    reader = CodeReader(payload[4:])

    def expand(initial, next_code):
        code = initial
        reversed_ = []
        while code > 0xFF:
            if code >= next_code:
                raise ValueError("invalid dictionary reference")
            reversed_.append(suffix[code])
            code = prefix[code]
        reversed_.append(code)
        reversed_.reverse()
        return reversed_

    prefix = [0] * 0x4000
    suffix = [0] * 0x4000
    next_code = 0x100
    old = reader.read()
    if old > 0xFF:
        raise ValueError("starts with dictionary code")
    output = [old]

    while True:
        code = reader.read()
        if code == 0x3FFF:
            break
        if code >= next_code:
            if code != next_code:
                raise ValueError("future dictionary reference")
            value = expand(old, next_code)
            value.append(value[0])
        else:
            value = expand(code, next_code)
        output.extend(value)
        if next_code <= 0x3FFE:
            prefix[next_code] = old
            suffix[next_code] = value[0]
            next_code += 1
        old = code

    if len(output) != expected:
        raise ValueError(
            f"decoded {len(output)} bytes but record declares {expected}"
        )
    return bytes(output)


def to_ascii(blob):
    return "".join(chr(b) for b in blob)


def main(path, out_path):
    data = open(path, "rb").read()
    nodes = archive_nodes(data)

    starts = sorted({offset for _, offset in nodes} | {len(data)})

    lines = [f"leaf records: {len(nodes)}"]
    for (leaf_path, offset) in sorted(nodes, key=lambda item: item[1]):
        end = next(s for s in starts if s > offset)
        rid = "/".join(f"{part:03}" for part in leaf_path)
        payload = data[offset:end]
        try:
            decoded = decompress(payload)
        except ValueError as exc:
            lines.append(f"{rid}  <ERROR: {exc}>")
            continue
        lines.append(f"{rid}: {to_ascii(decoded)}")

    with open(out_path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
    print(f"wrote {len(lines)} lines to {out_path}")


if __name__ == "__main__":
    import os

    if len(sys.argv) > 2:
        main(sys.argv[1], sys.argv[2])
    else:
        out = os.path.splitext(sys.argv[1] if len(sys.argv) > 1 else "strings.dat")[0]
        main(sys.argv[1] if len(sys.argv) > 1 else "strings.dat", out + "-ascii.txt")

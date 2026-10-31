#!/usr/bin/env python3
"""Exercise the exact Band 11 .179 fragment queue with dirty heap contents."""

import argparse
import hashlib
import struct
from pathlib import Path

from unicorn import Uc, UC_ARCH_ARM, UC_HOOK_MEM_WRITE, UC_MODE_THUMB
from unicorn.arm_const import UC_ARM_REG_LR, UC_ARM_REG_R0, UC_ARM_REG_R1, UC_ARM_REG_SP

IMAGE_BASE = 0x0C0C0000
IMAGE_SHA256 = "307adabe0ff61c77be73d045a8260e279cddd131c7a535a2678eb34199278a2b"
QUEUE_PUSH = 0x0C8610E6
QUEUE_POP = 0x0C8611D4
HEAP = 0x20000000
QUEUE = HEAP
NODE = HEAP + 0x100
STOP = HEAP + 0x800
POISON = HEAP + 0x4004


def call(machine, address, *arguments):
    for register, value in zip((UC_ARM_REG_R0, UC_ARM_REG_R1), arguments):
        machine.reg_write(register, value)
    machine.reg_write(UC_ARM_REG_LR, STOP | 1)
    machine.emu_start(address | 1, STOP)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", type=Path, required=True, help="Exact .179 vela_ap.bin")
    args = parser.parse_args()
    image = args.image.read_bytes()
    assert hashlib.sha256(image).hexdigest() == IMAGE_SHA256, "wrong firmware image"
    assert len(image) > QUEUE_POP - IMAGE_BASE + 34, "truncated firmware image"

    machine = Uc(UC_ARCH_ARM, UC_MODE_THUMB)
    machine.mem_map(IMAGE_BASE, (len(image) + 0xFFF) & ~0xFFF)
    machine.mem_write(IMAGE_BASE, image)
    machine.mem_map(HEAP, 0x5000)
    machine.reg_write(UC_ARM_REG_SP, HEAP + 0x2000)
    writes = []
    initial_sp = HEAP + 0x2000

    def record_write(machine, _access, address, size, value, _user):
        if not (initial_sp - 8 <= address < initial_sp):
            writes.append((address, size, value))

    machine.hook_add(UC_HOOK_MEM_WRITE, record_write)
    for address, length in ((QUEUE_PUSH, 32), (QUEUE_POP, 34)):
        expected = image[address - IMAGE_BASE : address - IMAGE_BASE + length]
        assert machine.mem_read(address, length) == expected

    def push_and_pop(header):
        machine.mem_write(QUEUE, b"\0" * 8)
        machine.mem_write(NODE - 8, struct.pack("<II", *header))
        writes.clear()
        call(machine, QUEUE_PUSH, QUEUE, NODE)
        call(machine, QUEUE_POP, QUEUE)
        assert machine.reg_read(UC_ARM_REG_R0) == NODE
        return list(writes)

    dirty = push_and_pop((POISON, POISON))
    assert (POISON - 4, 4, 0) in dirty, "firmware did not consume the dirty list link"
    assert struct.unpack("<I", machine.mem_read(QUEUE, 4))[0] == POISON

    cleared = push_and_pop((0, 0))
    allowed = {QUEUE, QUEUE + 4, NODE - 8, NODE - 4, NODE}
    assert all(address in allowed for address, _size, _value in cleared), (
        f"clean queue wrote unexpected memory: {cleared!r}"
    )
    assert machine.mem_read(QUEUE, 8) == b"\0" * 8
    print("PASS: exact .179 queue writes through a dirty link and drains a cleared node")


if __name__ == "__main__":
    main()

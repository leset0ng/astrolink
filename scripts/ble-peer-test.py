#!/usr/bin/env python3
"""Real H4/LE/L2CAP test equipment for Astrolink; standard library only.

The external socket is a controller's host H4 UART. It is never a socket to a
native module or a second controller UART. No API, SDU, or connection is faked.
Run --self-test without a controller. See docs/peer-test.md before a live run.
"""
from __future__ import annotations

import argparse
from collections import deque
from dataclasses import dataclass, field
import hashlib
import json
from pathlib import Path
import socket
import struct
import sys
import time
import unittest

UUID = bytes.fromhex('d594b03f621a678e5c4bd8f2a1e0439c')
SIGNAL_CID = 5
BOUNDARIES = (0, 1, 511, 512)


class ProtocolError(RuntimeError):
    pass


def u16(data: bytes, offset: int = 0) -> int:
    return struct.unpack_from('<H', data, offset)[0]


def address(data: bytes) -> str:
    return ':'.join(f'{byte:02X}' for byte in reversed(data))


def advertisement(service: bytes, spsm: int, user: bytes = b'\xa5') -> bytes:
    if not 1 <= len(service) <= 6 or not 0x80 <= spsm <= 0xFF or len(user) > 1:
        raise ValueError('service must be 1..6 UTF-8 bytes, SPSM 0x80..0xff, data <=1 byte')
    payload = bytes([0x21]) + UUID + bytes([1, spsm, len(service)]) + service + user
    return b'\x02\x01\x02' + bytes([len(payload)]) + payload


def parse_advertisement(data: bytes) -> tuple[bytes, int, bytes] | None:
    result = None
    offset = 0
    while offset < len(data):
        length = data[offset]
        if length == 0:
            break
        end = offset + 1 + length
        if end > len(data):
            return None
        ad = data[offset + 1:end]
        offset = end
        if ad[0] != 0x21 or len(ad) < 20 or ad[1:17] != UUID:
            continue
        if result is not None or ad[17] != 1 or ad[18] < 0x80:
            return None
        count = ad[19]
        if not 1 <= count <= 6 or 20 + count > len(ad) or len(ad) - 20 - count > 1:
            return None
        name = ad[20:20 + count]
        try:
            name.decode('utf-8')
        except UnicodeDecodeError:
            return None
        result = name, ad[18], ad[20 + count:]
    return result


class H4Parser:
    def __init__(self):
        self.buffer = bytearray()

    def feed(self, data: bytes) -> list[tuple[int, bytes]]:
        self.buffer.extend(data)
        packets = []
        while self.buffer:
            kind = self.buffer[0]
            if kind == 4:  # Event
                if len(self.buffer) < 3:
                    break
                length = 3 + self.buffer[2]
            elif kind == 2:  # ACL
                if len(self.buffer) < 5:
                    break
                length = 5 + u16(self.buffer, 3)
            else:
                raise ProtocolError(f'unsupported H4 packet type 0x{kind:02x}')
            if len(self.buffer) < length:
                break
            packets.append((kind, bytes(self.buffer[1:length])))
            del self.buffer[:length]
        return packets


class ACLReassembly:
    """Reassemble HCI ACL PB fragments into Basic L2CAP packets per handle."""
    def __init__(self):
        self.partial: dict[int, bytearray] = {}

    def feed(self, packet: bytes) -> tuple[int, int, bytes] | None:
        if len(packet) < 4 or len(packet) != 4 + u16(packet, 2):
            raise ProtocolError('malformed HCI ACL length')
        flags = u16(packet)
        handle, pb, bc = flags & 0xFFF, (flags >> 12) & 3, flags >> 14
        if bc:
            raise ProtocolError('broadcast ACL on an LE point-to-point link')
        payload = packet[4:]
        if pb in (0, 2, 3):
            if handle in self.partial:
                raise ProtocolError('new ACL first fragment interrupted an incomplete L2CAP packet')
            self.partial[handle] = bytearray(payload)
        elif handle in self.partial:
            self.partial[handle].extend(payload)
        else:
            raise ProtocolError('orphan ACL continuation')
        value = self.partial[handle]
        if len(value) < 4:
            if pb == 3:
                raise ProtocolError('complete ACL packet lacks the Basic L2CAP header')
            return None
        total = 4 + u16(value)
        if total > 65539 or len(value) > total:
            raise ProtocolError('ACL reassembly exceeds Basic L2CAP length')
        if pb == 3 and len(value) != total:
            raise ProtocolError('complete ACL PB flag carries an incomplete L2CAP packet')
        if len(value) != total:
            return None
        del self.partial[handle]
        return handle, u16(value, 2), bytes(value[4:])


def acl_fragments(handle: int, cid: int, payload: bytes, acl_mtu: int) -> list[bytes]:
    if not 0 < handle < 0xF00 or not 4 <= acl_mtu <= 65535 or len(payload) > 65535:
        raise ValueError('invalid ACL/L2CAP framing parameters')
    pdu = struct.pack('<HH', len(payload), cid) + payload
    return [b'\x02' + struct.pack('<HH', handle | (0 if offset == 0 else 0x1000),
                                len(pdu[offset:offset + acl_mtu])) + pdu[offset:offset + acl_mtu]
            for offset in range(0, len(pdu), acl_mtu)]


def sdu_fragments(data: bytes, mps: int) -> list[bytes]:
    if not 23 <= mps <= 65533 or len(data) > 65535:
        raise ValueError('invalid LE CoC SDU/MPS')
    first = struct.pack('<H', len(data)) + data[:mps - 2]
    remaining = data[mps - 2:]
    return [first] + [remaining[i:i + mps] for i in range(0, len(remaining), mps)]


class SDUReassembly:
    def __init__(self, mtu: int, mps: int):
        self.mtu, self.mps = mtu, mps
        self.expected: int | None = None
        self.data = bytearray()

    def feed(self, frame: bytes) -> bytes | None:
        if not frame or len(frame) > self.mps:
            raise ProtocolError('LE CoC K-frame outside receive MPS')
        if self.expected is None:
            if len(frame) < 2:
                raise ProtocolError('first LE CoC K-frame lacks SDU length')
            self.expected = u16(frame)
            if self.expected > self.mtu:
                raise ProtocolError('LE CoC SDU exceeds advertised receive MTU')
            self.data = bytearray(frame[2:])
        else:
            self.data.extend(frame)
        if len(self.data) > self.expected:
            raise ProtocolError('LE CoC reassembly exceeds SDU length')
        if len(self.data) != self.expected:
            return None
        result = bytes(self.data)
        self.expected = None
        self.data.clear()
        return result


def make_probe(sequence: int, length: int) -> bytes:
    if length < 9:
        raise ValueError('example probe needs 9 bytes')
    return b'ALQ\x01\x01' + struct.pack('<I', sequence) + bytes(
        (i * 73 + sequence) & 0xFF for i in range(9, length))


def probe_response(data: bytes) -> bytes | None:
    if len(data) >= 9 and data[:4] == b'ALQ\x01' and data[4] == 1:
        return data[:4] + b'\x02' + data[5:]
    return None


@dataclass
class Channel:
    local: int
    remote: int
    remote_mtu: int
    remote_mps: int
    tx_credits: int
    rx_credits: int
    assembler: SDUReassembly
    outgoing: deque[bytes] = field(default_factory=deque)
    hello_sent: bool = False
    hello_received: bool = False
    closed: bool = False


class Evidence:
    def __init__(self, path: Path):
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        self.events: list[dict] = []
        self.started = time.monotonic()
        self.summary: dict = {'kind': 'external-standard-le-peer', 'ok': False,
                              'physical_radio_verified': False}

    def emit(self, kind: str, **values):
        event = {'seconds': round(time.monotonic() - self.started, 6), 'kind': kind, **values}
        self.events.append(event)
        if len(self.events) > 20000:
            raise ProtocolError('evidence event limit reached')
        print(json.dumps(event, ensure_ascii=False), flush=True)

    def save(self):
        self.path.write_text(json.dumps({'summary': self.summary, 'events': self.events},
                                        ensure_ascii=False, indent=2) + '\n')


class PeerHost:
    def __init__(self, args, evidence: Evidence):
        self.args, self.log = args, evidence
        self.socket: socket.socket | None = None
        self.parser, self.acl = H4Parser(), ACLReassembly()
        self.command_results: deque[tuple[int, int, bytes]] = deque()
        self.acl_mtu, self.acl_packets = 27, 1
        self.acl_outstanding = 0
        self.acl_queue: deque[bytes] = deque()
        self.handle: int | None = None
        self.connection_seen = False
        self.connection_error: int | None = None
        self.advertisements: list[dict] = []
        self.target: tuple[int, bytes, int] | None = None
        self.channel: Channel | None = None
        self.pending_connect: int | None = None
        self.pending_disconnect: int | None = None
        self.signal_ident = 0
        self.service = args.service.encode('utf-8')
        self.local_address = b''
        self.tx_sizes: set[int] = set()
        self.rx_sizes: set[int] = set()
        self.verified_responses: set[int] = set()
        self.expected_responses: dict[int, bytes] = {}
        self.received_requests = 0
        self.scan_enabled = False
        self.advertising_enabled = False
        self.connection_pending = False
        self.remote_disconnected = False
        self.closing = False

    def deadline(self) -> float:
        return time.monotonic() + self.args.timeout

    def ident(self) -> int:
        self.signal_ident = self.signal_ident % 255 + 1
        return self.signal_ident

    def send_h4(self, packet: bytes):
        assert self.socket is not None
        self.socket.sendall(packet)
        self.log.emit('h4_tx', packet_type=packet[0], bytes=len(packet), hex=packet.hex())

    def flush(self):
        channel = self.channel
        if channel and not channel.closed and self.handle is not None:
            while channel.outgoing and channel.tx_credits:
                frame = channel.outgoing.popleft()
                channel.tx_credits -= 1
                self.queue_l2cap(channel.remote, frame)
        while self.acl_queue and self.acl_outstanding < self.acl_packets:
            packet = self.acl_queue.popleft()
            self.send_h4(packet)
            self.acl_outstanding += 1

    def pump(self, deadline: float):
        if time.monotonic() >= deadline:
            return
        self.flush()
        assert self.socket is not None
        self.socket.settimeout(min(0.2, max(0.001, deadline - time.monotonic())))
        try:
            data = self.socket.recv(65536)
        except socket.timeout:
            return
        if not data:
            raise ProtocolError('H4 controller endpoint closed')
        for kind, packet in self.parser.feed(data):
            self.log.emit('h4_rx', packet_type=kind, bytes=len(packet) + 1,
                          hex=(bytes([kind]) + packet).hex())
            if kind == 4:
                self.event(packet)
            else:
                pdu = self.acl.feed(packet)
                if pdu:
                    handle, cid, payload = pdu
                    if self.handle is None or handle != self.handle:
                        raise ProtocolError('ACL data does not belong to the established LE handle')
                    self.l2cap(cid, payload)
        self.flush()

    def wait(self, predicate, message: str, deadline: float | None = None):
        deadline = self.deadline() if deadline is None else deadline
        while not predicate():
            if time.monotonic() >= deadline:
                raise TimeoutError(message)
            if self.remote_disconnected and not self.closing:
                raise ProtocolError('peer disconnected before the requested result')
            self.pump(deadline)

    def command(self, opcode: int, params: bytes = b'') -> bytes:
        if len(params) > 255:
            raise ValueError('HCI command parameters exceed one octet')
        self.send_h4(b'\x01' + struct.pack('<HB', opcode, len(params)) + params)
        deadline = self.deadline()
        while True:
            for result in list(self.command_results):
                if result[0] == opcode:
                    self.command_results.remove(result)
                    _, status, returns = result
                    self.log.emit('command_result', opcode=f'0x{opcode:04x}', status=status,
                                  returns=returns.hex())
                    if status:
                        raise ProtocolError(f'HCI command 0x{opcode:04x} failed: 0x{status:02x}')
                    return returns
            if time.monotonic() >= deadline:
                raise TimeoutError(f'HCI command 0x{opcode:04x} completion/status timeout')
            self.pump(deadline)

    def event(self, packet: bytes):
        code, length = packet[:2]
        data = packet[2:]
        if len(data) != length:
            raise ProtocolError('HCI event length mismatch')
        if code == 0x0E:
            if len(data) < 4:
                raise ProtocolError('short Command Complete')
            self.command_results.append((u16(data, 1), data[3], data[4:]))
        elif code == 0x0F:
            if len(data) != 4:
                raise ProtocolError('malformed Command Status')
            self.command_results.append((u16(data, 2), data[0], b''))
        elif code == 0x13:
            if not data or len(data) != 1 + 4 * data[0]:
                raise ProtocolError('malformed Number Of Completed Packets')
            for offset in range(1, len(data), 4):
                if u16(data, offset) == self.handle:
                    count = u16(data, offset + 2)
                    if count > self.acl_outstanding:
                        raise ProtocolError('controller completed more ACL packets than submitted')
                    self.acl_outstanding -= count
        elif code == 0x05:
            if len(data) != 4:
                raise ProtocolError('malformed Disconnection Complete')
            if u16(data, 1) == self.handle and data[0] == 0:
                self.log.emit('le_disconnected', reason=data[3])
                self.handle = None
                self.acl_outstanding = 0
                self.acl_queue.clear()
                self.remote_disconnected = True
        elif code == 0x03:
            raise ProtocolError('BR/EDR connection event received: LE fallback is forbidden')
        elif code == 0x3E:
            self.le_event(data)
        elif code == 0x10:
            raise ProtocolError(f'controller Hardware Error: {data.hex()}')

    def le_event(self, data: bytes):
        if not data:
            raise ProtocolError('empty LE Meta event')
        sub = data[0]
        if sub in (1, 0x0A):
            if len(data) != (19 if sub == 1 else 31):
                raise ProtocolError('malformed LE Connection Complete event')
            self.connection_pending = False
            if data[1]:
                self.connection_error = data[1]
                self.log.emit('le_connection_failed', status=data[1])
                return
            if self.handle is not None:
                raise ProtocolError('unexpected second LE connection')
            self.handle = u16(data, 2)
            self.connection_seen = True
            expected_role = 0 if self.args.role == 'central' else 1
            if data[4] != expected_role or self.handle == 0:
                raise ProtocolError('LE event role/handle does not match requested GAP procedure')
            if self.target and (data[5], data[6:12]) != self.target[:2]:
                raise ProtocolError('LE event peer identity differs from selected advertisement')
            self.log.emit('le_connected', handle=self.handle, role=data[4],
                          address_type=data[5], peer=address(data[6:12]), enhanced=sub == 0x0A)
        elif sub == 2:
            if len(data) < 2:
                raise ProtocolError('short LE Advertising Report')
            offset = 2
            for _ in range(data[1]):
                if offset + 10 > len(data):
                    raise ProtocolError('short LE advertising report record')
                event_type, addr_type = data[offset:offset + 2]
                addr = data[offset + 2:offset + 8]
                count = data[offset + 8]
                end = offset + 9 + count
                if end >= len(data):
                    raise ProtocolError('truncated LE advertisement data/RSSI')
                self.advertisement_report(event_type, addr_type, addr, data[offset + 9:end],
                                          struct.unpack('b', data[end:end + 1])[0])
                offset = end + 1
            if offset != len(data):
                raise ProtocolError('trailing bytes in LE Advertising Report')
        elif sub == 0x0D:
            if len(data) < 2:
                raise ProtocolError('short LE Extended Advertising Report')
            offset = 2
            for _ in range(data[1]):
                if offset + 24 > len(data):
                    raise ProtocolError('short extended advertisement record')
                event_type = u16(data, offset)
                addr_type = data[offset + 2]
                addr = data[offset + 3:offset + 9]
                count = data[offset + 23]
                end = offset + 24 + count
                if end > len(data):
                    raise ProtocolError('truncated extended advertisement')
                if event_type & 0x60 == 0:  # Only complete reports, never partial data.
                    self.advertisement_report(event_type, addr_type, addr,
                                              data[offset + 24:end],
                                              struct.unpack('b', data[offset + 13:offset + 14])[0],
                                              extended=True)
                offset = end
            if offset != len(data):
                raise ProtocolError('trailing extended advertisement bytes')

    def advertisement_report(self, event_type: int, addr_type: int, addr: bytes, raw: bytes, rssi: int,
                             extended: bool = False):
        decoded = parse_advertisement(raw)
        if decoded is None or decoded[0] != self.service:
            return
        if self.args.peer and address(addr).lower() != self.args.peer.lower():
            return
        if len(self.advertisements) < 128:
            self.advertisements.append({'peer': address(addr), 'address_type': addr_type,
                                        'spsm': decoded[1], 'rssi': rssi})
        # Legacy ADV_IND(0) / ADV_DIRECT_IND(1) are connectable. Extended report
        # sets bit0 connectable; scan responses alone cannot select a target.
        connectable = bool(event_type & 1) if extended else event_type in (0, 1)
        self.log.emit('astrolink_advertisement', peer=address(addr), address_type=addr_type,
                      spsm=decoded[1], service=self.args.service, data=decoded[2].hex(), rssi=rssi,
                      event_type=event_type, connectable=connectable)
        if connectable and addr_type in (0, 1) and self.target is None:
            self.target = addr_type, addr, decoded[1]

    def queue_l2cap(self, cid: int, payload: bytes):
        if self.handle is None:
            raise ProtocolError('cannot send L2CAP before LE connection establishment')
        self.acl_queue.extend(acl_fragments(self.handle, cid, payload, self.acl_mtu))
        if len(self.acl_queue) > 256:
            raise ProtocolError('bounded host ACL queue overflow')

    def signal(self, code: int, ident: int, payload: bytes):
        if not ident:
            raise ProtocolError('L2CAP signaling identifier zero')
        self.queue_l2cap(SIGNAL_CID, struct.pack('<BBH', code, ident, len(payload)) + payload)
        self.log.emit('l2cap_signal_tx', code=f'0x{code:02x}', ident=ident, data=payload.hex())

    def l2cap(self, cid: int, payload: bytes):
        if cid == SIGNAL_CID:
            offset = 0
            while offset < len(payload):
                if offset + 4 > len(payload):
                    raise ProtocolError('truncated L2CAP signaling header')
                code, ident, length = struct.unpack_from('<BBH', payload, offset)
                offset += 4
                if ident == 0 or offset + length > len(payload):
                    raise ProtocolError('invalid L2CAP signaling identifier/length')
                self.signaling(code, ident, payload[offset:offset + length])
                offset += length
        elif self.channel and cid == self.channel.local and not self.channel.closed:
            channel = self.channel
            if channel.rx_credits == 0:
                raise ProtocolError('firmware exceeded granted CoC receive credits')
            channel.rx_credits -= 1
            data = channel.assembler.feed(payload)
            if channel.rx_credits <= self.args.credits // 2:
                added = self.args.credits - channel.rx_credits
                # CID denotes the grant sender's local endpoint. The recipient
                # looks it up as its remote CID; see Linux v6.14 l2cap_le_credits.
                self.signal(0x16, self.ident(), struct.pack('<HH', channel.local, added))
                channel.rx_credits += added
            if data is not None:
                self.received_sdu(data)
        elif cid == 6:
            raise ProtocolError('SMP pairing requested; this unauthenticated test profile does not negotiate security')
        else:
            self.log.emit('unhandled_l2cap', cid=cid, length=len(payload))

    def open_channel(self, remote: int, mtu: int, mps: int, credits: int):
        if not 0x40 <= remote <= 0x7F or mtu < 23 or not 23 <= mps <= 65533:
            raise ProtocolError('invalid legacy LE CoC channel parameters')
        self.channel = Channel(0x40, remote, mtu, mps, credits, self.args.credits,
                               SDUReassembly(self.args.mtu, self.args.mps))
        self.log.emit('coc_open', local_cid=0x40, remote_cid=remote, local_receive_mtu=self.args.mtu,
                      local_receive_mps=self.args.mps, remote_receive_mtu=mtu,
                      remote_receive_mps=mps, initial_tx_credits=credits)
        self.queue_sdu(bytes([1, len(self.service)]) + self.service, hello=True)

    def signaling(self, code: int, ident: int, payload: bytes):
        self.log.emit('l2cap_signal_rx', code=f'0x{code:02x}', ident=ident, data=payload.hex())
        if code == 0x14:
            if len(payload) != 10:
                raise ProtocolError('invalid LE Credit Based Connection Request size')
            psm, remote, mtu, mps, credits = struct.unpack('<HHHHH', payload)
            if psm != self.args.spsm:
                self.signal(0x15, ident, struct.pack('<HHHHH', 0, 0, 0, 0, 2))
            elif self.channel and not self.channel.closed:
                self.signal(0x15, ident, struct.pack('<HHHHH', 0, 0, 0, 0, 4))
            elif not 0x40 <= remote <= 0x7F:
                self.signal(0x15, ident, struct.pack('<HHHHH', 0, 0, 0, 0, 9))
            elif mtu < 23 or not 23 <= mps <= 65533:
                self.signal(0x15, ident, struct.pack('<HHHHH', 0, 0, 0, 0, 11))
            else:
                self.signal(0x15, ident, struct.pack('<HHHHH', 0x40, self.args.mtu,
                                                  self.args.mps, self.args.credits, 0))
                self.open_channel(remote, mtu, mps, credits)
        elif code == 0x15:
            if len(payload) != 10 or ident != self.pending_connect:
                raise ProtocolError('unexpected/malformed LE CoC connection response')
            remote, mtu, mps, credits, result = struct.unpack('<HHHHH', payload)
            self.pending_connect = None
            if result:
                raise ProtocolError(f'LE CoC request rejected: 0x{result:04x}')
            self.open_channel(remote, mtu, mps, credits)
        elif code == 0x16:
            if len(payload) != 4 or not self.channel:
                raise ProtocolError('unexpected/malformed LE Flow Control Credit indication')
            cid, credits = struct.unpack('<HH', payload)
            if cid != self.channel.remote or self.channel.tx_credits + credits > 65535:
                raise ProtocolError('LE credit CID mismatch or credit overflow')
            self.channel.tx_credits += credits
        elif code == 6:
            if len(payload) != 4:
                raise ProtocolError('malformed L2CAP Disconnection Request')
            dcid, scid = struct.unpack('<HH', payload)
            if self.channel and (dcid, scid) == (self.channel.local, self.channel.remote):
                self.signal(7, ident, payload)
                self.channel.closed = True
                self.log.emit('coc_disconnected', initiator='firmware')
            else:
                self.signal(1, ident, struct.pack('<HHH', 2, dcid, scid))
        elif code == 7:
            if len(payload) != 4 or ident != self.pending_disconnect or not self.channel:
                raise ProtocolError('unexpected L2CAP Disconnection Response')
            if struct.unpack('<HH', payload) != (self.channel.remote, self.channel.local):
                raise ProtocolError('L2CAP disconnect CID mismatch')
            self.channel.closed = True
            self.pending_disconnect = None
            self.log.emit('coc_disconnected', initiator='test-host')
        elif code == 0x12:
            if len(payload) != 8:
                raise ProtocolError('malformed Connection Parameter Update Request')
            # This test does not run the Link Layer update procedure. Rejecting
            # optional parameters is standard and never pretends they changed.
            self.signal(0x13, ident, struct.pack('<H', 1))
        elif code == 1:
            raise ProtocolError(f'firmware rejected L2CAP signaling ident={ident}: {payload.hex()}')
        elif code == 0x13:
            if len(payload) != 2:
                raise ProtocolError('malformed Connection Parameter Update Response')
            self.log.emit('connection_parameter_response', data=payload.hex())
        else:
            self.signal(1, ident, struct.pack('<H', 0))

    def queue_sdu(self, data: bytes, hello=False):
        channel = self.channel
        if channel is None or channel.closed or len(data) > channel.remote_mtu:
            raise ProtocolError('SDU cannot be submitted to this CoC channel/remote MTU')
        frames = sdu_fragments(data, channel.remote_mps)
        if len(channel.outgoing) + len(frames) > 64:
            raise ProtocolError('bounded outgoing CoC queue overflow')
        channel.outgoing.extend(frames)
        if hello:
            channel.hello_sent = True
            self.log.emit('hello_tx', data=data.hex())
        else:
            self.tx_sizes.add(len(data))
            self.log.emit('sdu_tx_queued', length=len(data), sha256=hashlib.sha256(data).hexdigest())

    def received_sdu(self, data: bytes):
        channel = self.channel
        assert channel is not None
        if not channel.hello_received:
            if data != bytes([1, len(self.service)]) + self.service:
                raise ProtocolError('Astrolink HELLO version/service mismatch')
            channel.hello_received = True
            self.log.emit('hello_rx', data=data.hex())
            return
        self.rx_sizes.add(len(data))
        self.log.emit('sdu_rx', length=len(data), sha256=hashlib.sha256(data).hexdigest(),
                      data_hex=data.hex() if len(data) <= 16 else None)
        response = probe_response(data)
        if response is not None:
            self.received_requests += 1
            self.queue_sdu(response)
            self.log.emit('example_request_echoed', sequence=struct.unpack_from('<I', data, 5)[0], length=len(data))
        elif len(data) >= 9 and data[:5] == b'ALQ\x01\x02':
            sequence = struct.unpack_from('<I', data, 5)[0]
            expected = self.expected_responses.pop(sequence, None)
            if expected is not None:
                if data != expected:
                    raise ProtocolError(f'example binary response mismatch for sequence {sequence}')
                self.verified_responses.add(len(data))
                self.log.emit('example_response_verified', sequence=sequence, length=len(data))

    def initialize(self):
        self.socket = socket.create_connection((self.args.host, self.args.port), self.args.timeout)
        self.command(0x0C03)
        # Enable only defined events used by this host; reserved mask bits stay zero.
        event_mask = sum(1 << (event - 1) for event in (5, 8, 14, 15, 16, 19, 26, 62))
        le_mask = sum(1 << (event - 1) for event in (1, 2, 3, 4, 5, 6, 7, 10, 13, 18))
        self.command(0x0C01, struct.pack('<Q', event_mask))
        self.command(0x2001, struct.pack('<Q', le_mask))
        local = self.command(0x1009)
        if len(local) != 6:
            raise ProtocolError('Read BD_ADDR did not return 6 bytes')
        self.local_address = local
        result = self.command(0x2002)
        if len(result) != 3:
            raise ProtocolError('invalid LE Read Buffer Size response')
        self.acl_mtu, self.acl_packets = u16(result), result[2]
        if self.acl_mtu == 0:
            result = self.command(0x1005)
            if len(result) != 7:
                raise ProtocolError('invalid shared Read Buffer Size response')
            self.acl_mtu, self.acl_packets = u16(result), u16(result, 3)
        if self.acl_mtu < 4 or self.acl_packets == 0:
            raise ProtocolError('controller has no usable LE ACL buffers')
        self.log.emit('controller_ready', own_address=address(local), acl_mtu=self.acl_mtu,
                      acl_packets=self.acl_packets, role=self.args.role)

    def establish(self):
        if self.args.role == 'peripheral':
            params = struct.pack('<HHBBB6sBB', 0xA0, 0xA0, 0, 0, 0, b'\0' * 6, 7, 0)
            self.command(0x2006, params)
            raw = advertisement(self.service, self.args.spsm)
            self.command(0x2008, bytes([len(raw)]) + raw.ljust(31, b'\0'))
            self.command(0x200A, b'\x01')
            self.advertising_enabled = True
            self.log.emit('ready_for_quickapp', action='start scan, select this peer and connect',
                          own_address=address(self.local_address), spsm=self.args.spsm)
        else:
            self.command(0x200B, struct.pack('<BHHBB', 1, 0x30, 0x30, 0, 0))
            self.command(0x200C, b'\x01\x00')
            self.scan_enabled = True
            self.log.emit('ready_for_quickapp', action='start advertising')
            self.wait(lambda: self.target is not None, 'no complete matching connectable Astrolink advertisement')
            self.command(0x200C, b'\0\0')
            self.scan_enabled = False
            assert self.target is not None
            kind, peer, _ = self.target
            self.connection_pending = True
            if self.args.extended:
                params = struct.pack('<BBB6sB', 0, 0, kind, peer, 1) + struct.pack(
                    '<HHHHHHHH', 0x30, 0x30, 24, 40, 0, 500, 0, 0)
                self.command(0x2043, params)
            else:
                params = struct.pack('<HHBB6sBHHHHHH', 0x30, 0x30, 0, kind, peer,
                                     0, 24, 40, 0, 500, 0, 0)
                self.command(0x200D, params)
        self.wait(lambda: self.handle is not None or self.connection_error is not None,
                  'no LE Connection Complete (Command Status alone does not establish a link)')
        if self.connection_error is not None:
            raise ProtocolError(f'LE connection failed: 0x{self.connection_error:02x}')
        if self.args.role == 'central':
            self.pending_connect = self.ident()
            self.signal(0x14, self.pending_connect, struct.pack('<HHHHH', self.target[2], 0x40,
                                                             self.args.mtu, self.args.mps, self.args.credits))
        self.wait(lambda: self.channel is not None and self.channel.hello_received,
                  'CoC open/Astrolink HELLO timeout')
        self.log.emit('astrolink_ready', service=self.args.service)

    def exercise(self):
        if not self.args.listen_only:
            self.queue_sdu(b'')
            self.queue_sdu(b'\xa5')
            for sequence, length in [(0xA11001, 511), (0xA11002, 512)]:
                request = make_probe(sequence, length)
                self.expected_responses[sequence] = probe_response(request)
                self.queue_sdu(request)
        self.log.emit('ready_for_quickapp', action='run Ping / boundary tests; send actual 0/1/511/512-byte SDUs',
                      duration_seconds=self.args.duration)
        deadline = time.monotonic() + self.args.duration
        while time.monotonic() < deadline and self.handle is not None and not self.channel.closed:
            self.pump(deadline)
            complete = set(BOUNDARIES).issubset(self.rx_sizes)
            echoed = self.args.listen_only or {511, 512}.issubset(self.verified_responses)
            submitted = not self.acl_queue and not self.channel.outgoing and self.acl_outstanding == 0
            if complete and echoed and submitted and not self.args.hold:
                break
        self.log.summary.update({
            'le_connection_observed': self.connection_seen,
            'hello_verified': self.channel.hello_received,
            'host_queued_sdu_sizes': sorted(self.tx_sizes),
            'firmware_to_peer_sdu_sizes': sorted(self.rx_sizes),
            'byte_verified_example_response_sizes': sorted(self.verified_responses),
            'firmware_example_requests_echoed': self.received_requests,
            'missing_firmware_sdu_sizes': sorted(set(BOUNDARIES) - self.rx_sizes),
            'outgoing_kframes_pending': len(self.channel.outgoing),
            'outgoing_acl_pending': len(self.acl_queue),
            'controller_acl_outstanding': self.acl_outstanding,
            'zero_one_receive_evidence': 'HCI reception proves firmware-to-peer delivery; '
                'peer-to-firmware 0/1-byte app consumption requires QuickApp counters/logs',
        })
        if not set(BOUNDARIES).issubset(self.rx_sizes):
            raise ProtocolError('boundary run incomplete: no real firmware SDUs at every required size')
        if not self.args.listen_only and not {511, 512}.issubset(self.verified_responses):
            raise ProtocolError('511/512-byte example request/response verification incomplete')
        if self.channel.outgoing or self.acl_queue or self.acl_outstanding:
            raise ProtocolError('outgoing traffic was not fully handed off/completed by controller')
        # Transport observation is explicit; it cannot assert JS consumption.
        self.log.summary['ok'] = True

    def cleanup(self):
        self.closing = True
        errors = []
        if self.socket is None:
            return errors
        if self.handle is not None and self.channel and not self.channel.closed:
            try:
                self.pending_disconnect = self.ident()
                self.signal(6, self.pending_disconnect, struct.pack('<HH', self.channel.remote, self.channel.local))
                self.wait(lambda: self.channel.closed or self.handle is None,
                          'L2CAP Disconnection Response timeout', time.monotonic() + min(3, self.args.timeout))
            except (OSError, RuntimeError, TimeoutError) as error:
                errors.append(str(error))
        if self.handle is not None:
            try:
                self.command(0x0406, struct.pack('<HB', self.handle, 0x13))
                self.wait(lambda: self.handle is None, 'HCI disconnect timeout',
                          time.monotonic() + min(3, self.args.timeout))
            except (OSError, RuntimeError, TimeoutError) as error:
                errors.append(str(error))
        elif self.connection_pending:
            try:
                self.command(0x200E)
            except (OSError, RuntimeError, TimeoutError) as error:
                errors.append(str(error))
        for enabled, opcode, value in [(self.scan_enabled, 0x200C, b'\0\0'),
                                       (self.advertising_enabled, 0x200A, b'\0')]:
            if enabled:
                try:
                    self.command(opcode, value)
                except (OSError, RuntimeError, TimeoutError) as error:
                    errors.append(str(error))
        self.socket.close()
        self.socket = None
        self.log.emit('cleanup', errors=errors)
        return errors


class FramingTests(unittest.TestCase):
    def test_advertisement_matches_rust_and_rejects_all_truncations(self):
        raw = advertisement(b'k7m2p9', 0xFF)
        self.assertEqual(len(raw), 31)
        self.assertEqual(raw[:5], b'\x02\x01\x02\x1b\x21')
        self.assertEqual(parse_advertisement(raw), (b'k7m2p9', 0xFF, b'\xa5'))
        for end in range(len(raw)):
            self.assertIsNone(parse_advertisement(raw[:end]))
        self.assertIsNone(parse_advertisement(raw + raw[3:]))

    def test_h4_incremental_and_concatenated_packets(self):
        raw = b'\x04\x0e\x04\x01\x03\x0c\x00' + b'\x02\x01\x00\x04\x00test'
        parser = H4Parser()
        results = []
        for byte in raw:
            results += parser.feed(bytes([byte]))
        self.assertEqual(results, [(4, raw[1:7]), (2, raw[8:])])
        self.assertEqual(parser.buffer, b'')
        with self.assertRaises(ProtocolError):
            parser.feed(b'\xff')

    def test_acl_fragments_interleave_by_handle_without_sdu_confusion(self):
        receiver = ACLReassembly()
        a = acl_fragments(1, 0x40, bytes(range(200)), 27)
        b = acl_fragments(2, 5, b'signal', 5)
        observed = []
        while a or b:
            for queue in (a, b):
                if queue:
                    value = receiver.feed(queue.pop(0)[1:])
                    if value:
                        observed.append(value)
        self.assertCountEqual(observed, [(1, 0x40, bytes(range(200))), (2, 5, b'signal')])
        self.assertFalse(receiver.partial)

    def test_orphan_overlong_and_interrupted_acl_are_rejected(self):
        with self.assertRaises(ProtocolError):
            ACLReassembly().feed(struct.pack('<HH', 0x1001, 1) + b'x')
        with self.assertRaises(ProtocolError):
            ACLReassembly().feed(struct.pack('<HHHH', 1, 5, 0, 5) + b'x')
        receiver = ACLReassembly()
        packet = acl_fragments(1, 5, b'abcdef', 5)[0][1:]
        receiver.feed(packet)
        with self.assertRaises(ProtocolError):
            receiver.feed(packet)

    def test_coc_boundaries_and_many_mps_sizes(self):
        for mps in (23, 64, 128, 512):
            for length in BOUNDARIES:
                data = bytes(i & 0xFF for i in range(length))
                frames = sdu_fragments(data, mps)
                receiver = SDUReassembly(512, mps)
                complete = []
                for frame in frames:
                    self.assertLessEqual(len(frame), mps)
                    result = receiver.feed(frame)
                    if result is not None:
                        complete.append(result)
                self.assertEqual(complete, [data])
                self.assertIsNone(receiver.expected)

    def test_sdu_receive_limits_and_empty_sdu(self):
        self.assertEqual(SDUReassembly(512, 23).feed(b'\0\0'), b'')
        for frame in (b'\0', b'\x01\0xx', b'\x01\x02', b''):
            with self.assertRaises(ProtocolError):
                SDUReassembly(512, 23).feed(frame)
        with self.assertRaises(ProtocolError):
            SDUReassembly(512, 23).feed(b'x' * 24)

    def host(self):
        class Args:
            service = 'chat'
            spsm = 0x80
            mtu = 512
            mps = 23
            credits = 4
            role = 'central'
            peer = None
        class Log:
            def emit(self, *_args, **_kwargs):
                pass
        host = PeerHost(Args(), Log())
        host.handle = 1
        host.channel = Channel(0x40, 0x41, 512, 64, 0, 2, SDUReassembly(512, 23),
                               hello_sent=True, hello_received=True)
        return host

    def test_credit_cid_is_remote_on_receipt_and_local_when_granted(self):
        host = self.host()
        host.signaling(0x16, 7, struct.pack('<HH', 0x41, 3))
        self.assertEqual(host.channel.tx_credits, 3)
        with self.assertRaises(ProtocolError):
            host.signaling(0x16, 8, struct.pack('<HH', 0x40, 1))
        host.l2cap(0x40, b'\x00\x00')
        grant = ACLReassembly().feed(host.acl_queue[0][1:])
        self.assertEqual(grant[1], 5)
        code, _ident, length, cid, credits = struct.unpack('<BBHHH', grant[2])
        self.assertEqual((code, length, cid, credits), (0x16, 4, 0x40, 3))

    def test_credit_overflow_and_zero_credit_receive_are_protocol_errors(self):
        host = self.host()
        host.channel.tx_credits = 65535
        with self.assertRaises(ProtocolError):
            host.signaling(0x16, 1, struct.pack('<HH', 0x41, 1))
        host.channel.rx_credits = 0
        with self.assertRaises(ProtocolError):
            host.l2cap(0x40, b'\0\0')

    def test_connection_and_disconnect_signaling_use_endpoint_cids(self):
        host = self.host()
        host.channel = None
        host.signaling(0x14, 9, struct.pack('<HHHHH', 0x80, 0x41, 512, 64, 3))
        response = ACLReassembly().feed(host.acl_queue[0][1:])
        self.assertEqual(response[1], 5)
        self.assertEqual(response[2], struct.pack('<BBHHHHHH', 0x15, 9, 10, 0x40, 512, 23, 4, 0))
        self.assertEqual((host.channel.local, host.channel.remote), (0x40, 0x41))
        host.signaling(6, 10, struct.pack('<HH', 0x40, 0x41))
        self.assertTrue(host.channel.closed)
        response = ACLReassembly().feed(host.acl_queue[-1][1:])
        self.assertEqual(response[2], struct.pack('<BBHHH', 7, 10, 4, 0x40, 0x41))

    def test_hci_status_does_not_establish_connection(self):
        host = self.host()
        host.handle = None
        host.event(b'\x0f\x04\x00\x01\x0d\x20')
        self.assertIsNone(host.handle)
        self.assertFalse(host.connection_seen)
        legacy = struct.pack('<BBHBB6sHHHB', 1, 0, 1, 0, 1, b'123456', 24, 0, 500, 0)
        host.event(bytes([0x3e, len(legacy)]) + legacy)
        self.assertEqual(host.handle, 1)
        self.assertTrue(host.connection_seen)
        with self.assertRaises(ProtocolError):
            host.event(b'\x03\x00')

    def test_example_echo_is_request_only_and_byte_preserving(self):
        for length in (9, 511, 512):
            request = make_probe(0xA11001, length)
            response = probe_response(request)
            self.assertEqual(response[:4] + b'\x01' + response[5:], request)
            self.assertIsNone(probe_response(response))
        self.assertIsNone(probe_response(b''))
        self.assertIsNone(probe_response(b'\xa5'))


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--host', default='127.0.0.1')
    parser.add_argument('--port', type=int)
    parser.add_argument('--role', choices=['central', 'peripheral'], default='peripheral')
    parser.add_argument('--service', default='chat')
    parser.add_argument('--spsm', type=lambda value: int(value, 0), default=0x80)
    parser.add_argument('--peer', help='central mode: optional AP address filter, e.g. AA:BB:CC:DD:EE:FF')
    parser.add_argument('--mtu', type=int, default=512)
    parser.add_argument('--mps', type=int, default=128)
    parser.add_argument('--credits', type=int, default=16)
    parser.add_argument('--timeout', type=float, default=30, help='maximum seconds per procedure')
    parser.add_argument('--duration', type=float, default=60, help='maximum seconds of established SDU exercise')
    parser.add_argument('--extended', action='store_true', help='central: use LE Extended Create Connection')
    parser.add_argument('--listen-only', action='store_true', help='only respond to real QuickApp requests; do not initiate boundary sends')
    parser.add_argument('--hold', action='store_true', help='keep responding until duration even after all checks pass')
    parser.add_argument('--output', type=Path, default=Path('/tmp/astrolink-le-peer.json'))
    args = parser.parse_args()
    if not args.self_test:
        try:
            advertisement(args.service.encode('utf-8'), args.spsm)
        except ValueError as error:
            parser.error(str(error))
        if args.port is None or not 1 <= args.port <= 65535:
            parser.error('--port 1..65535 is required for a live controller')
        if not 512 <= args.mtu <= 65535 or not 23 <= args.mps <= 65533:
            parser.error('boundary equipment requires MTU >=512 and MPS 23..65533')
        if not 2 <= args.credits <= 65535 or not 0 < args.timeout <= 120 or not 0 < args.duration <= 600:
            parser.error('credits 2..65535, timeout (0,120], duration (0,600] required')
    return args


def main():
    args = arguments()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(FramingTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    evidence = Evidence(args.output)
    evidence.summary.update({'role': args.role, 'service': args.service, 'spsm': args.spsm,
                             'endpoint': f'{args.host}:{args.port}',
                             'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
    peer = PeerHost(args, evidence)
    exit_code = 0
    try:
        peer.initialize()
        peer.establish()
        peer.exercise()
    except (OSError, RuntimeError, TimeoutError, ValueError) as error:
        evidence.summary['error'] = str(error)
        evidence.emit('failure', error=str(error))
        exit_code = 1
    finally:
        try:
            cleanup_errors = peer.cleanup()
        except (OSError, RuntimeError, TimeoutError, ValueError) as error:
            cleanup_errors = [str(error)]
        evidence.summary.setdefault('le_connection_observed', peer.connection_seen)
        evidence.summary.setdefault('host_queued_sdu_sizes', sorted(peer.tx_sizes))
        evidence.summary.setdefault('firmware_to_peer_sdu_sizes', sorted(peer.rx_sizes))
        evidence.summary['cleanup_errors'] = cleanup_errors
        if cleanup_errors:
            evidence.summary['ok'] = False
            exit_code = 1
        evidence.save()
        print(json.dumps({'summary': evidence.summary, 'evidence': str(args.output)}, ensure_ascii=False))
    return exit_code


if __name__ == '__main__':
    sys.exit(main())

/** Example-only echo envelope. The native Link still carries one SDU per send. */
const HEADER_BYTES = 9
const REQUEST = 1
const RESPONSE = 2

export interface ProbePacket {
  response: boolean
  sequence: number
  bytes: Uint8Array
}

export function makeProbe(sequence: number, size: number): Uint8Array {
  if (size < HEADER_BYTES || !Number.isSafeInteger(size)) throw new Error("消息上限不足以发送测试包")
  const bytes = new Uint8Array(size)
  bytes[0] = 0x41
  bytes[1] = 0x4c
  bytes[2] = 0x51
  bytes[3] = 1
  bytes[4] = REQUEST
  new DataView(bytes.buffer).setUint32(5, sequence, true)
  for (let i = HEADER_BYTES; i < size; i++) bytes[i] = (i * 73 + sequence) & 0xff
  return bytes
}

export function parseProbe(data: ArrayBuffer): ProbePacket | null {
  const bytes = new Uint8Array(data)
  if (bytes.length < HEADER_BYTES || bytes[0] !== 0x41 || bytes[1] !== 0x4c ||
      bytes[2] !== 0x51 || bytes[3] !== 1 || (bytes[4] !== REQUEST && bytes[4] !== RESPONSE)) return null
  return { response: bytes[4] === RESPONSE, sequence: new DataView(data).getUint32(5, true), bytes }
}

export function makeResponse(packet: ProbePacket): Uint8Array {
  const bytes = new Uint8Array(packet.bytes.length)
  bytes.set(packet.bytes)
  bytes[4] = RESPONSE
  return bytes
}

export function verifyResponse(expected: Uint8Array, received: Uint8Array): boolean {
  if (expected.length !== received.length) return false
  for (let i = 0; i < expected.length; i++) {
    if (received[i] !== (i === 4 ? RESPONSE : expected[i])) return false
  }
  return true
}

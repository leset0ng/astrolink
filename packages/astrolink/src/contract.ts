/** Astrolink's public native contract. No alternate transport is used. */
export type Unsubscribe = () => void
export interface Capabilities {
  supported: boolean
  maxLinks: number
  maxServiceIdBytes: number
  maxAdvertisingDataBytes: number
}
export interface Advertisement {
  peerId: string
  serviceId: string
  data: ArrayBuffer
  rssi: number
}
export interface Link {
  readonly id: string
  readonly peerId: string
  readonly maxMessageBytes: number
  onData(callback: (data: ArrayBuffer) => void): Unsubscribe
  onDisconnect(callback: (reason: string) => void): Unsubscribe
  send(data: ArrayBuffer | Uint8Array): Promise<void>
  disconnect(): Promise<void>
}
export interface Session {
  onAdvertisement(callback: (event: Advertisement) => void): Unsubscribe
  startScan(options?: { allowDuplicates?: boolean }): Promise<void>
  stopScan(): Promise<void>
  startAdvertising(options?: { data?: ArrayBuffer | Uint8Array }): Promise<void>
  stopAdvertising(): Promise<void>
  onConnection(callback: (link: Link) => void): Unsubscribe
  connect(peerId: string): Promise<Link>
  close(): Promise<void>
}
export interface Astrolink {
  getCapabilities(): Capabilities
  createSession(options: { serviceId: string }): Session
}

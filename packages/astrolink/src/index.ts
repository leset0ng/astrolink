import type { Astrolink } from "./contract.js"
import { loadAstrolink } from "./native.js"

export type { Advertisement, Astrolink, Capabilities, Link, Session, Unsubscribe } from "./contract.js"
export type { NativeEvent, PrimitiveFeature } from "./facade.js"
export { AstrolinkFacade, AstrolinkError, EVENT, OP } from "./facade.js"
export { loadAstrolink, disposeAstrolink } from "./native.js"
export { utf8ByteLength } from "./utf8.js"

// Importing the package never requires the native Feature. Its first operation does.
const astrolink: Astrolink = {
  getCapabilities: () => loadAstrolink().getCapabilities(),
  createSession: options => loadAstrolink().createSession(options)
}
export default astrolink

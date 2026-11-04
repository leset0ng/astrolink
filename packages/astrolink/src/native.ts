import { AstrolinkFacade } from "./facade.js"
import type { PrimitiveFeature } from "./facade.js"

// Confirmed Q66 Feature name. The owner context supplies this native loader.
declare const $app_require$: (name: string) => PrimitiveFeature
// Legacy hosts publish on the engine global; QuickApp shadows globalThis.
declare const __canopusAstrolinkNative: PrimitiveFeature | undefined

type NativeGlobal = typeof globalThis & {
  __canopusAstrolinkFacade?: AstrolinkFacade
  __canopusAstrolinkNative?: PrimitiveFeature
  aiot?: { canopusAstrolink?: PrimitiveFeature }
}
const nativeGlobal = globalThis as NativeGlobal

export function loadAstrolink(): AstrolinkFacade {
  // Both generated app/page bundles share this slot and one primitive subscription.
  if (!nativeGlobal.__canopusAstrolinkFacade) {
    const primitive = nativeGlobal.aiot?.canopusAstrolink
      ?? nativeGlobal.__canopusAstrolinkNative
      ?? (typeof __canopusAstrolinkNative === "undefined" ? undefined : __canopusAstrolinkNative)
      ?? $app_require$("@app-module/canopus.astrolink")
    nativeGlobal.__canopusAstrolinkFacade = new AstrolinkFacade(primitive)
  }
  return nativeGlobal.__canopusAstrolinkFacade
}

export function disposeAstrolink(): Promise<void> {
  const facade = nativeGlobal.__canopusAstrolinkFacade
  if (!facade) return Promise.resolve()
  return facade.dispose().finally(() => {
    if (nativeGlobal.__canopusAstrolinkFacade === facade) delete nativeGlobal.__canopusAstrolinkFacade
  })
}

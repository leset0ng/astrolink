import { AstrolinkFacade } from "./facade.js"
import type { PrimitiveFeature } from "./facade.js"

// Confirmed Q66 Feature name. The owner context supplies this native loader.
declare const $app_require$: (name: string) => PrimitiveFeature

type NativeGlobal = typeof globalThis & { __canopusAstrolinkFacade?: AstrolinkFacade }
const nativeGlobal = globalThis as NativeGlobal

export function loadAstrolink(): AstrolinkFacade {
  // Both generated app/page bundles share this slot and one primitive subscription.
  if (!nativeGlobal.__canopusAstrolinkFacade) {
    nativeGlobal.__canopusAstrolinkFacade = new AstrolinkFacade($app_require$("@app-module/canopus.astrolink"))
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

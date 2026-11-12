import { TestController } from "./controller"
import { loadAstrolink, disposeAstrolink } from "@lesetong/astrolink"

// The toolkit emits app.js and page.js as separate bundles. Their module-local
// variables cannot share Session ownership, so keep one app-global controller.
type ExampleGlobal = typeof globalThis & { __astrolinkExampleController?: TestController }
const appGlobal = globalThis as ExampleGlobal

export function getController(): TestController {
  let controller = appGlobal.__astrolinkExampleController
  if (!controller) {
    controller = new TestController(loadAstrolink)
    appGlobal.__astrolinkExampleController = controller
    controller.initialize()
  }
  return controller
}

export function destroyController(): void {
  const active = appGlobal.__astrolinkExampleController
  delete appGlobal.__astrolinkExampleController
  if (active) void active.destroy()
  void disposeAstrolink().catch(error => console.error("[Astrolink] Feature cleanup failed", error))
}

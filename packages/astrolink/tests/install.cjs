const fs = require("node:fs")
const os = require("node:os")
const path = require("node:path")
const assert = require("node:assert/strict")
const { execFileSync } = require("node:child_process")
const root = path.resolve(__dirname, "..")
const tarball = path.join(root, "dist/lesetong-astrolink-0.1.1.tgz")
const consumer = fs.mkdtempSync(path.join(os.tmpdir(), "astrolink-npm-install-"))
try {
  fs.writeFileSync(path.join(consumer, "package.json"), JSON.stringify({ name: "astrolink-install-smoke", private: true, version: "1.0.0" }))
  execFileSync("npm", ["install", tarball, "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--package-lock=false"],
    { cwd: consumer, stdio: "inherit" })
  const installed = JSON.parse(fs.readFileSync(path.join(consumer, "node_modules/@lesetong/astrolink/package.json"), "utf8"))
  assert.equal(installed.name, "@lesetong/astrolink")
  assert.equal(installed.dependencies, undefined)
  assert.equal(fs.existsSync(path.join(consumer, "node_modules/@lesetong/astrolink/src")), false)
  fs.writeFileSync(path.join(consumer, "check.cjs"), `
    const assert = require("node:assert/strict")
    const pkg = require("@lesetong/astrolink")
    assert.equal(typeof pkg.default.createSession, "function")
    assert.equal(typeof pkg.loadAstrolink, "function")
    let callback, subscribed = 0, unsubscribed = 0
    globalThis.$app_require$ = name => {
      assert.equal(name, "@app-module/canopus.astrolink")
      return {
        getCapabilities: () => ({ supported: true, maxLinks: 1, maxServiceIdBytes: 6, maxAdvertisingDataBytes: 1 }),
        createSession: service => { assert.equal(service, "chat"); return 1 },
        subscribe: cb => { callback = cb; if (cb) subscribed++; else unsubscribed++; return 0 },
        request: (op, session, token) => {
          queueMicrotask(() => callback({ kind: 1, session, token, error: 0 }))
          return 0
        }
      }
    }
    ;(async () => {
      const session = pkg.default.createSession({ serviceId: "chat" })
      await session.startScan()
      await session.close()
      await pkg.disposeAstrolink()
      assert.equal(subscribed, 1)
      assert.equal(unsubscribed, 1)
      console.log("Installed CommonJS consumer passed")
    })().catch(error => { console.error(error); process.exitCode = 1 })
  `)
  execFileSync(process.execPath, ["check.cjs"], { cwd: consumer, stdio: "inherit" })
  fs.writeFileSync(path.join(consumer, "check.mjs"), `
    import assert from "node:assert/strict"
    import astrolink, { loadAstrolink, utf8ByteLength } from "@lesetong/astrolink"
    assert.equal(typeof astrolink.createSession, "function")
    assert.equal(typeof loadAstrolink, "function")
    assert.equal(utf8ByteLength("中文"), 6)
    console.log("Installed ESM consumer passed without requiring a native Feature")
  `)
  execFileSync(process.execPath, ["check.mjs"], { cwd: consumer, stdio: "inherit" })
  fs.writeFileSync(path.join(consumer, "check.mts"), `
    import astrolink, { loadAstrolink, type Session, type Link, type Capabilities } from "@lesetong/astrolink"
    const capabilities: Capabilities = astrolink.getCapabilities()
    const session: Session = loadAstrolink().createSession({ serviceId: "chat" })
    const connection: Promise<Link> = session.connect("00112233445500")
    session.onConnection(link => { void link.send(new Uint8Array([0, 255])) })
    void capabilities; void connection; void session.close()
  `)
  execFileSync(process.execPath, [require.resolve("typescript/bin/tsc"), "--noEmit", "--strict", "--skipLibCheck",
    "--target", "ES2015", "--lib", "ES2020,DOM", "--module", "NodeNext", "--moduleResolution", "NodeNext", "check.mts"],
    { cwd: consumer, stdio: "inherit" })
  fs.writeFileSync(path.join(consumer, "check.cts"), `
    import astrolink = require("@lesetong/astrolink")
    const session: astrolink.Session = astrolink.default.createSession({ serviceId: "chat" })
    void session.close()
  `)
  execFileSync(process.execPath, [require.resolve("typescript/bin/tsc"), "--noEmit", "--strict", "--skipLibCheck",
    "--target", "ES2015", "--lib", "ES2020,DOM", "--module", "NodeNext", "--moduleResolution", "NodeNext", "check.cts"],
    { cwd: consumer, stdio: "inherit" })
  console.log("Installed TypeScript ESM and CommonJS consumers passed")
} finally {
  fs.rmSync(consumer, { recursive: true, force: true })
}

const fs = require("node:fs")
const path = require("node:path")
const { execFileSync } = require("node:child_process")
const os = require("node:os")
const variant = process.argv[2]
if (!["source", "bytecode", "probe"].includes(variant)) throw new Error("expected source, bytecode or probe")
const root = path.resolve(__dirname, "..")
const dist = path.join(root, "dist")
// The toolkit replaces dist wholesale; keep all named package variants.
const previous = new Map()
for (const name of ["astrolink-example-source.rpk", "astrolink-example-bytecode.rpk", "astrolink-probe-source.rpk"]) {
  const file = path.join(dist, name)
  if (fs.existsSync(file)) previous.set(name, fs.readFileSync(file))
}
let temporary
try {
  let buildRoot = root
  if (variant === "probe") {
    temporary = fs.mkdtempSync(path.join(os.tmpdir(), "astrolink-probe-build-"))
    buildRoot = temporary
    fs.cpSync(path.join(root, "src"), path.join(buildRoot, "src"), { recursive: true })
    for (const name of ["package.json", "quickapp.config.js", "jsconfig.json", "tsconfig.json"]) {
      fs.copyFileSync(path.join(root, name), path.join(buildRoot, name))
    }
    fs.symlinkSync(path.join(root, "node_modules"), path.join(buildRoot, "node_modules"), "dir")
    const manifestPath = path.join(buildRoot, "src/manifest.json")
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"))
    manifest.package = "com.canopus.astrolink.probe"
    manifest.router.entry = "pages/probe"
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n")
  }
  execFileSync(process.execPath, [require.resolve("aiot-toolkit/lib/bin.js"), "build",
    ...(variant === "bytecode" ? ["--enable-jsc"] : [])], { cwd: buildRoot, stdio: "inherit" })
  const manifest = JSON.parse(fs.readFileSync(path.join(buildRoot, "src/manifest.json"), "utf8"))
  const version = manifest.versionName || "0.0.0"
  const built = path.join(buildRoot, "dist", `${manifest.package}.debug.${version}.rpk`)
  // Read before restoring preserved outputs, including the variant being rebuilt.
  const bytes = fs.readFileSync(built)
  fs.mkdirSync(dist, { recursive: true })
  for (const [name, previousBytes] of previous) fs.writeFileSync(path.join(dist, name), previousBytes)
  const output = path.join(dist, variant === "probe" ? "astrolink-probe-source.rpk" : `astrolink-example-${variant}.rpk`)
  fs.writeFileSync(output, bytes)
  console.log(`Preserved ${output}`)
} finally {
  if (temporary) fs.rmSync(temporary, { recursive: true, force: true })
}

const fs = require("node:fs")
const path = require("node:path")
const { execFileSync } = require("node:child_process")
const variant = process.argv[2]
if (variant !== "source" && variant !== "bytecode") throw new Error("expected source or bytecode")
const root = path.resolve(__dirname, "..")
const dist = path.join(root, "dist")
// The toolkit replaces dist wholesale; keep both actual package variants.
const previous = new Map()
for (const name of ["astrolink-example-source.rpk", "astrolink-example-bytecode.rpk"]) {
  const file = path.join(dist, name)
  if (fs.existsSync(file)) previous.set(name, fs.readFileSync(file))
}
execFileSync(process.execPath, [require.resolve("aiot-toolkit/lib/bin.js"), "build",
  ...(variant === "bytecode" ? ["--enable-jsc"] : [])], { cwd: root, stdio: "inherit" })
for (const [name, bytes] of previous) fs.writeFileSync(path.join(dist, name), bytes)
const output = path.join(dist, `astrolink-example-${variant}.rpk`)
fs.copyFileSync(path.join(dist, "com.canopus.astrolink.example.debug.0.1.0.rpk"), output)
console.log(`Preserved ${output}`)

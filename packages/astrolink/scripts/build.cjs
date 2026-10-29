const fs = require("node:fs")
const path = require("node:path")
const { execFileSync } = require("node:child_process")
const root = path.resolve(__dirname, "..")
const compiler = require.resolve("typescript/bin/tsc")
for (const [module, out] of [["ES2015", "esm"], ["CommonJS", "cjs"]]) {
  const output = path.join(root, "dist", out)
  fs.rmSync(output, { recursive: true, force: true })
  execFileSync(process.execPath, [compiler, "-p", "tsconfig.json", "--module", module, "--outDir", output],
    { cwd: root, stdio: "inherit" })
  const extension = out === "esm" ? ".mjs" : ".cjs"
  for (const name of fs.readdirSync(output).filter(name => name.endsWith(".js"))) {
    const input = path.join(output, name)
    const source = fs.readFileSync(input, "utf8").replace(/(\.\/[a-z0-9-]+)\.js(["'])/g, `$1${extension}$2`)
    fs.writeFileSync(path.join(output, name.replace(/\.js$/, extension)), source)
    fs.unlinkSync(input)
  }
}
fs.rmSync(path.join(root, "dist/types"), { recursive: true, force: true })
execFileSync(process.execPath, [compiler, "-p", "tsconfig.json", "--module", "ES2015", "--declaration",
  "--emitDeclarationOnly", "--outDir", "dist/types"], { cwd: root, stdio: "inherit" })

const cjsTypes = path.join(root, "dist/types-cjs")
fs.rmSync(cjsTypes, { recursive: true, force: true })
fs.mkdirSync(cjsTypes, { recursive: true })
for (const name of fs.readdirSync(path.join(root, "dist/types"))) {
  const source = fs.readFileSync(path.join(root, "dist/types", name), "utf8")
    .replace(/(\.\/[a-z0-9-]+)\.js(["'])/g, "$1.cjs$2")
  fs.writeFileSync(path.join(cjsTypes, name.replace(/\.d\.ts$/, ".d.cts")), source)
}

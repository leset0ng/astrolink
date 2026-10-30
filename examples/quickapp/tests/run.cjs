const { execFileSync } = require("node:child_process")
const path = require("node:path")
const root = path.resolve(__dirname, "..")
execFileSync(process.execPath, [require.resolve("typescript/bin/tsc"),
  "--target", "ES2020", "--module", "commonjs", "--strict", "--skipLibCheck",
  "--outDir", ".test-build", "src/ts/controller.ts", "src/ts/probe.ts"
], { cwd: root, stdio: "inherit" })
execFileSync(process.execPath, ["--test", "tests/controller.test.cjs", "tests/probe-page.test.cjs"], { cwd: root, stdio: "inherit" })

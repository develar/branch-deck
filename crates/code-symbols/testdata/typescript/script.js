const { join } = require("path")

export function main(args) {
  const out = join(...args)
  console.log(out, VERSION)
}

# Start the globally selected Yarn with Node.js from PATH

Manage Yarn Classic 1.x from the official `yarn` npm package and modern Yarn 2+
from the official `@yarnpkg/cli-dist` npm package, accepting stable releases with
registry SHA-512 integrity metadata. This leaves some early modern releases
unavailable rather than falling back to an unverified standalone script.

Start the selected Yarn's official entry point with Node.js from `PATH` and
inherit the caller's environment. This keeps Verslot's role limited to version
management and lets Yarn apply its own project configuration, including
`yarn-path`, `yarnPath` and `packageManager` checks. The global selection is the
starting Yarn release. A project can delegate execution to another release.

See [Issue #30](https://github.com/verslot/verslot/issues/30) for the scope and
acceptance criteria.

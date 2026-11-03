# Run the globally selected Yarn with the selected Node.js

Manage Yarn Classic 1.x from the official `yarn` npm package and modern Yarn 2+
from the official `@yarnpkg/cli-dist` npm package, accepting stable releases with
registry SHA-512 integrity metadata. This leaves some early modern releases
unavailable rather than falling back to an unverified standalone script.

Run the selected Yarn's JavaScript entry point directly with the globally
selected Node.js, without Corepack or a system Node fallback. Set
`YARN_IGNORE_PATH=1` so project `yarnPath` / `yarn-path` configuration cannot
replace the selected Yarn; preserve Yarn's own `packageManager` compatibility
checks. This gives the global selection a consistent meaning at the cost of
project-local Yarn delegation. Installing and selecting Yarn do not require
Node.js; executing it does. Each Yarn release retains its own Node.js runtime
requirements.

See [Issue #30](https://github.com/verslot/verslot/issues/30) for the scope and
acceptance criteria.

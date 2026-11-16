# Verslot

Verslot is a tool version manager for Node.js and Yarn, written in Rust.

Download Verslot from [GitHub Releases](https://github.com/verslot/verslot/releases).

## Quick start

```text
verslot install node@24
verslot use node@24
```

After installing the first version of a tool, rerun `setup` from the installed
Verslot executable to add that tool's shims. Then run `node --version` to check
the selection. Use `yarn@<version>` in the same commands to manage Yarn, which
requires Node.js on `PATH`.

Run `verslot --help` for commands or `verslot doctor` to diagnose selections and
PATH.

[Changelog](CHANGELOG.md) · [Report an issue](https://github.com/verslot/verslot/issues) · [MIT License](LICENSE)

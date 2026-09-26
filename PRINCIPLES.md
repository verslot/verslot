# Verslot Principles

## Rust First

Verslot core must be implemented in Rust.

## Version Management Only

The core only manages tool versions.

Core responsibilities:
- discover versions
- install versions
- uninstall versions
- select project versions
- select global versions
- resolve active versions
- execute the selected tool version

Non-version-management functionality must not enter the core.

## Extensible, Not Bloated

Additional functionality may be provided through optional extensions.

Extensions depend on the core.
The core must never depend on extensions.

## No Personal Branding

Verslot is infrastructure.

No:
- personal branding
- mascots
- author signatures in CLI output
- sponsorship messages in normal CLI output
- telemetry by default

## Engineering

Prefer:
- simple over clever
- explicit over magical
- stable over feature-rich
- standard library over unnecessary dependencies
- boring Rust over advanced Rust techniques

# Match remote versions by textual prefix

Remote version discovery matches numeric query text against the beginning of a
release's version number. We choose textual prefix matching over component
equality so `18.1` intentionally matches both `18.1.0` and `18.10.0`, keeping
partial and complete query inputs consistent. Remote query prefixes are discovery
filters.

The feature contract and acceptance criteria are tracked in
[Issue #21](https://github.com/verslot/verslot/issues/21).

Partial installation and selection requests compare complete numeric components,
so `node@18.1` resolves within `18.1.x`; selection resolves from local installations
and installation resolves from remotely available releases. This supersedes the
original exact-only installation input restriction under
[Issue #27](https://github.com/verslot/verslot/issues/27), while preserving a complete
resolved target and the remote query prefix decision above.

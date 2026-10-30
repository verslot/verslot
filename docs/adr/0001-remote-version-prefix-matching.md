# Match remote versions by textual prefix

Remote version discovery matches numeric query text against the beginning of a
release's version number. We choose textual prefix matching over component
equality so `18.1` intentionally matches both `18.1.0` and `18.10.0`, keeping
partial and complete query inputs consistent. Remote query prefixes are discovery
filters; installation targets denote exact versions.

The feature contract and acceptance criteria are tracked in
[Issue #21](https://github.com/verslot/verslot/issues/21).

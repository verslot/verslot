# Verslot

Verslot manages tool versions. This glossary defines release and version-request
terms used in discovery, installation and selection.

## Language

**Version request**:
A tool name with one to three numeric version components, identifying an exact
release or a major/minor release series whose supplied components must match.

**Resolved target**:
A tool and complete major, minor and patch version identifying one release.

**Official release**:
A Node.js version identified by major, minor, and patch numbers, without a
prerelease suffix or build metadata. Official releases include LTS and non-LTS
versions, regardless of whether they are still maintained.

**Remotely available version**:
An official release for which Node.js lists a distribution archive for the
current platform. Remote availability is independent of whether the version is
installed locally.

**Prefix filter**:
A restriction on remote version discovery that keeps releases whose version
number begins with the specified text. For example, `18.1` matches both `18.1.0`
and `18.10.0`.
_Avoid_: Major version filter

**LTS release**:
An official release marked as Long Term Support in Node.js's release catalog.
The marker applies to that particular release and does not imply that its release
series is currently maintained.
_Avoid_: Currently supported release

# Verslot

Verslot manages tool versions. This glossary defines managed-tool, selection,
release and version-request terms used in discovery, installation and selection.

## Language

**Managed tool**:
Named software whose installed versions and global selection Verslot manages
independently of other tools.

**Global selection**:
The installed version selected for a managed tool across directories until the
user changes it. Each tool has its own selection.

**Bundled command**:
A command supplied with a managed tool version, such as npm or npx in a Node.js
distribution. It follows the supplying tool's selection rather than having an
independent Verslot selection.

**Command shim**:
A Verslot-managed executable entry point in the shared PATH directory for the
Verslot CLI or a supported tool command.

**Version request**:
A tool name with one to three numeric version components, identifying an exact
release or a major/minor release series whose supplied components must match.

**Resolved target**:
A tool and complete major, minor and patch version identifying one release.

**Official release**:
A managed tool's published version identified by major, minor, and patch numbers,
without a prerelease suffix or build metadata. Official releases include versions
that are no longer maintained.

**Remotely available version**:
An official release with a distribution available through the tool's supported
official source for the current platform. Remote availability is independent of
whether the version is installed locally.

**Prefix filter**:
A restriction on remote version discovery that keeps releases whose version
number begins with the specified text. For example, `18.1` matches both `18.1.0`
and `18.10.0`.
_Avoid_: Major version filter

**LTS release**:
A Node.js official release marked as Long Term Support in its release catalog.
The marker applies to that particular release and does not imply that its release
series is currently maintained.
_Avoid_: Currently supported release

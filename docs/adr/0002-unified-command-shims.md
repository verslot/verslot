# Provide a single PATH through command shims

Use `<storage>/shims` for both `verslot` and installed tool commands,
replacing tool-specific PATH entries so adding a tool never requires another
PATH change. Each managed command uses that tool's global selection and fails
when the selection cannot be resolved, without falling back to system commands
or installing on demand. This adds launch-time dispatch in exchange for a stable
entry point across tools; legacy PATH compatibility is not a requirement during
early development.

Create and refresh entry points explicitly with repeatable `verslot setup`.
Include `verslot` and the commands for every tool with at least one complete
local installation, with or without a global selection. Use the same inventory
validation as `verslot list` and preserve the published shims when inventory
validation fails. Rerun setup after a tool's first installation or last-version
uninstall to synchronize its entries. This keeps commands for uninstalled tools
out of the shared directory and separates executable replacement from version
management. Once a shim is removed, the shell follows ordinary PATH lookup.
See [Issue #42](https://github.com/verslot/verslot/issues/42).

Resolve selections under a short-lived storage lock and release it before
executing the tool, allowing long-running programs and version management to
coexist. Switching affects subsequent launches; explicitly uninstalling a
version after switching away does not protect processes still using that
installation.

Stage one Verslot executable in the candidate shim directory and hard-link the
tool command names to it. Keeping the file and its links on the target volume
lets setup run from another volume while retaining native entry points and
refreshing them together. Use copies where hard links are unavailable.
See [Issue #34](https://github.com/verslot/verslot/issues/34).

The feature contract and acceptance criteria are tracked in
[Issue #29](https://github.com/verslot/verslot/issues/29); Yarn follows separately
in [Issue #30](https://github.com/verslot/verslot/issues/30).

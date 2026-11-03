# Provide a single PATH through command shims

Use `<storage>/shims` for both `verslot` and explicitly supported tool commands,
replacing tool-specific PATH entries so adding a tool never requires another
PATH change. Each managed command uses that tool's global selection and fails
when the selection cannot be resolved, without falling back to system commands
or installing on demand. This adds launch-time dispatch in exchange for a stable
entry point across tools; legacy PATH compatibility is not a requirement during
early development.

Create and refresh entry points explicitly with repeatable `verslot setup`,
independently of installed tool versions, so version selection does not require
replacing executable shims. Resolve selections under a short-lived storage lock
and release it before executing the tool, allowing long-running programs and
version management to coexist. Switching affects subsequent launches; explicitly
uninstalling a version after switching away does not protect processes still
using that installation.

The feature contract and acceptance criteria are tracked in
[Issue #29](https://github.com/verslot/verslot/issues/29); Yarn follows separately
in [Issue #30](https://github.com/verslot/verslot/issues/30).

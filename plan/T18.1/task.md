# T18.1. Workspace and trust model

`kerf-workspace`: recent folders (stored outside the repository), opening a folder, the Workspace Trust prompt's contents (language servers, debug adapters, tasks and project plugins that would start, with their commands verbatim), restricted mode, and revoking trust, on top of `kerf-core`'s trust store. Done when tests open trusted, untrusted and remembered folders, build the prompt from manifests, and see revoke report what to stop; the same through `kerf_workspace_*`.

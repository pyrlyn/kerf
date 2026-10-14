---
title: Kerf
tagline: An IDE with a small Rust core and sandboxed WASM plugins; LSP and DAP run as processes the core starts.
repo: https://github.com/pyrlyn/kerf
install: "git clone https://github.com/pyrlyn/kerf"
version: "0.0.0"
status: "Research · no code yet"
accent: "#C7D36F"
order: 12
---

<!-- Website copy for the pyrlyn project site, copied to pyrlyn/landing as
content/projects/kerf.md; front matter follows CONTENT_CONTRACT.md in that repository.
Sources (checked 2026-10-14): README.md and research.md; version from the workspace Cargo.toml (no code and no release yet, hence status). The accent is a placeholder: the repository has no brand colour yet. -->

## Overview

Kerf is an IDE with a small Rust core and sandboxed WASM plugins. The core draws the editor, holds
the text buffer, runs the event loop and keeps a semantic index of the code; languages and
debugging come in through LSP and DAP, as separate processes the core starts on a plugin's request.

Kerf is in the research stage: there is no code yet, only the research and the plan.

## Features

- **Small core.** Editor, text buffer, event loop and a semantic index (tree-sitter plus an incremental index).
- **Sandboxed plugins.** WASM plugins declare their permissions in a manifest and cannot spawn processes.
- **LSP and DAP as processes.** The core starts language and debug servers on a plugin's request.

## Install

There is nothing to install yet. Read the research:

```bash
git clone https://github.com/pyrlyn/kerf
```

## Usage examples

Kerf has no code yet; the research compares Zed, VS Code, IntelliJ, Visual Studio, Lapce, Neovim and others:

```bash
git clone https://github.com/pyrlyn/kerf && less kerf/research.md
```

## Links

- Repository: <https://github.com/pyrlyn/kerf>
- Research: <https://github.com/pyrlyn/kerf/blob/main/research.md>
- License: your choice of GNU GPLv3, a royalty-free license, or a commercial license (see <https://github.com/pyrlyn/kerf#license>)

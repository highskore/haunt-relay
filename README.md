<div align="center">

<img src="docs/icon.png" width="96" alt="">

# Relay

**Pack an agent's finished turn into a handoff brief for another agent.**

[![Haunt plugin](https://img.shields.io/badge/Haunt-plugin-D97757)](https://github.com/highskore/haunt-hackathon)
![Plugin API 0.5](https://img.shields.io/badge/plugin%20API-0.5-111)
![WASM component](https://img.shields.io/badge/WASM-component-654ff0?logo=webassembly&logoColor=white)
![MIT licence](https://img.shields.io/badge/licence-MIT-6b7280)

</div>

Type `relay` and part of a tab's name in the ⌘K palette (`relay api`). Each matching agent gets three rows:

- `api: last turn → brief`, with `14 files · 5 tasks`: a Markdown brief with the turn's goal (its prompt), the session's tasks as a checklist, the files it touched and its last plan.
- `api: plan only`, with the plan's length: only the plan.
- `api: files touched only`, with the path count: the file list, relative to the repo.

A new prompt starts the files over; the tasks and the last plan carry over until the session restarts. Agent text is cleaned of control and invisible characters before it goes into a brief.

Choosing a row copies that brief to the clipboard, line breaks and all. Haunt says so at the foot of the sidebar, and pasting more than one line of it into a Haunt terminal shows Haunt's paste check first.

## Settings

```toml
[plugins."dev.haunt.relay"]
unverified = false  # also use events without the tab's nonce
```

## Capabilities

| capability | why |
|---|---|
| `events` (session start, prompt, task, file edited, plan) | to build each tab's last turn |
| `event_details` | the prompt, plan and file paths in the brief; haunt asks in red, since they can hold secrets |
| `clipboard = "write"` | copies the brief you choose; only right after you choose it, never on its own |

Events without the tab's nonce are ignored unless `unverified` is on, since anything printed in the terminal can forge them.

## Build and test

```sh
cargo build --release --target wasm32-wasip2   # or ./build.sh
cargo test                                     # unit tests
haunt plugin install .
```

`e2e/relay.rs` runs the built plugin against a real Haunt core: copy it into a Haunt checkout's `cli/tests/` and run `HAUNT_PLUGINS_DIR=<this repo's parent> cargo test -p haunt-cli --features plugins --test relay`.

Needs Haunt with plugin API 0.5 (`api = "0.5"`).

## Licence

MIT, see `LICENSE`.

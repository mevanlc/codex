# Upstream overlap notes

## Merge of `88235f881d` into `21b175a582` (2026-09-27)

These notes record feature overlaps encountered during this merge, not a complete
audit of every fork difference. Commit IDs identify the incoming implementation.

| Area | Incoming upstream change | Merge decision |
| --- | --- | --- |
| Reasoning shortcuts | `bf832f4678` lets Alt+. and Shift+Up reach Max in default and Plan modes. | Max is now shared behavior. Preserve the fork's additional ability to reach Ultra, including the Plan-mode concurrency warning. Retain upstream's Max tests and the fork's advanced-effort tests. |
| GPT-6 model catalog | `49e95cc73f` duplicates the fork's `f1b21bb293` backport; `24462234b2`, `df30941072`, and `694d8d45bd` update metadata, Bedrock catalogs, and retired-model migrations. | Use the incoming catalog and provider-aware migration implementation. The earlier backport no longer needs a separate implementation. |
| Pending input | `04fc75adbe` preserves queued input during conditional interruption; `e7f119819a` adds input provenance and acceptance-order metadata; `1bf73324ca` adds optional Code Mode yielding. | These do not replace pending-steer retraction. Keep retraction under the active-turn/queue locks and carry the new metadata on retractable inputs. |
| Configuration loading | `22a3f6d5d8` and `9d4d34d436` enforce application network policy during loading and embedded startup. | Keep both policy enforcement and fork-overlay migration. Migration still precedes loading user configuration. |
| External editor | `d5355e95ef` keeps Codex visible during editor handoff and restores terminal modes afterward. | Keep the shared handoff improvements and the fork's Alt+G quoted-response buffer. Ctrl+G remains the ordinary editor binding. |
| Queue-edit hints | `591ffb1aae` consistently prefers Shift-arrow hints. | Adopt upstream hint ordering and remove obsolete terminal-dependent hint setup; retain the hint for retractable steers as well as locally queued messages. |
| Clipboard | `12fd929f72` moves copying off the UI path. | Adopt the new implementation and remove the obsolete stderr-suppression helper; its Android warning workaround is no longer needed. |

The fork's accent, placeholder-tip option, file-mention settings, shell follow-ups,
quoted editor, configuration overlay, and Android packaging remain distinct.

### Local validation

The six affected crates (`codex-tui`, `codex-core`, `codex-app-server`,
`codex-app-server-protocol`, `codex-config`, and `codex-models-manager`) compiled.
Of 12,994 tests, 12,943 passed initially. A serial retry of all 51 failures passed
48, including every TUI retry and the Code Mode yielding tests after rebuilding
the previously stale `codex-code-mode-host` helper. Across both runs, 12,991 tests
passed; 37 tests were skipped in the broad run.

Three checks remain unresolved on this machine:

- `optional_mcp_startup_grace_controls_initial_turn_tool_catalog::zero_grace_respects_server_startup_timeout`
  reaches its five-second initialization deadline, including in the serial retry.
- `astra_kickoff_with_skills_plugins_and_remote_compaction` and
  `astra_refreshes_plugin_tools_and_skills_in_an_existing_thread` discover the
  installed `open-pencil` skill, which changes their context snapshots. Those
  machine-specific snapshots were not accepted.

Config and experimental app-server schemas were regenerated, and the Bazel lock
refresh completed. Scoped Clippy passed after removing two unused imports.
Validation was local to macOS; no full-workspace or Android test run was performed.

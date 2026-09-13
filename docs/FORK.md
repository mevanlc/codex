# Working on the fork

This fork tracks [openai/codex](https://github.com/openai/codex). It was maintained for more than six months without a special merge-conflict-reduction discipline, and that is useful perspective: ordinary, clear code is a good default.

This guide offers considerations, not additional requirements or a checklist. When several implementations are roughly equivalent in clarity, correctness, performance, and effort, their likely upstream merge friction can be a useful tie-breaker. It is not a reason to contort the code, postpone useful work, or turn a small change into an architecture project. If the obvious implementation edits an upstream-owned function, that may be exactly the right implementation.

## Think about overlap, not just diff size

Merge friction comes partly from both sides editing the same places, and partly from having to reconcile different assumptions about how a feature works. A clean textual merge can still be wrong. Conversely, a small, obvious conflict can be easy to resolve and preferable to a clever abstraction that nobody wants to maintain.

The useful question is often: “What would someone need to understand to carry this behavior through an upstream change?” A compact integration point and a cohesive implementation can help. Fewer changed files or lines, by themselves, do not establish that a design is better.

For a routine change, local context is usually enough. When an area repeatedly causes trouble, its recent history and the fork's diff against the last merged upstream revision can reveal which code is actually shared and changing. There is no need for a repository-wide divergence audit before every feature.

## Look for natural boundaries

Self-contained fork behavior can often live in its own module while using the existing upstream machinery. Examples in this tree include:

- [Quoted editor buffers](../codex-rs/tui/src/quoted_editor_buffer.rs): preparing the editable draft and removing the quoted response are separate from launching the external editor.
- [Explicit-path search](../codex-rs/tui/src/file_search/explicit_paths.rs): query preparation and result projection are separate from search orchestration and stale-result handling.
- [Fork TUI options](../codex-rs/config/src/fork_tui.rs): related options are grouped internally while remaining flattened into the existing `[tui]` configuration table.

These are examples of useful boundaries, not templates that every feature needs to follow. A new helper, crate, trait, configuration group, or generic hook has its own cost. One direct conditional may be clearer than any of them. An existing extension point is attractive when it fits the behavior; inventing a framework solely to avoid touching upstream usually is not.

Ownership matters more than isolation. Pending-message retraction belongs with the real queue and its synchronization. Shell follow-up metadata belongs with the queued action. Search scope belongs with the request and results. Moving those concerns into globals, side tables, or UI-only state could reduce visible overlap while making the program harder to reason about.

Similarly, moving a whole upstream state machine into a fork-named file does not remove the work of integrating its future fixes. It mostly changes where that work happens. Small fork-specific extractions are often easier to justify than rearranging upstream code around them.

## Keep useful upstream structure and useful fork behavior

When updating shared code, following its existing organization, naming, and conventions tends to make both review and later merging easier. Unrelated reformatting, renaming, or reordering can obscure the actual behavioral change. A worthwhile refactor is still worthwhile; separating it from a feature change can make the reasoning easier to follow.

An upstream addition may make a fork patch unnecessary. Comparing the actual behavior, defaults, platforms, and tests is more informative than comparing feature names. Once it really covers the need, adopting the upstream implementation can retire both code and future reconciliation work.

Similar-looking behavior is not necessarily equivalent. For example, this fork captures mouse events and translates wheel events inside Codex because terminal-managed wheel-to-arrow translation has not worked reliably across the maintainer's terminal setups. A smaller diff would not compensate for losing that reliability. Removing or changing a feature is a product decision, not incidental merge cleanup.

## Make tests explain the difference

Fork-specific tests can often sit in dedicated files near their implementation, keeping large upstream test modules less crowded. The composer file-mention tests and core steer-retraction tests use this approach. Existing upstream tests can stay where they are unless there is another reason to move them; preserving snapshot names during an extraction also avoids unrelated churn.

Tests at integration boundaries remain valuable even when the implementation is neatly separated. They show that the fork behavior still participates correctly in upstream's queue, configuration, protocol, or rendering flow after a merge.

Incidental snapshot inputs are another opportunity. This tree uses a stable test version in [the TUI version module](../codex-rs/tui/src/version.rs), and fixtures can supply explicit terminal capabilities instead of depending on the developer's terminal. Stabilizing irrelevant inputs before rendering avoids version-dependent padding and terminal-dependent hints. That is different from filtering away meaningful output changes or weakening assertions to keep snapshots unchanged; behavior that depends on those inputs still needs representative coverage.

## Let generated files and build settings follow their sources

Schemas, protocol exports, and lockfiles reflect other decisions. Reconciling the source definitions or dependencies first, then using the repository's generators and tooling, is generally easier to reason about than hand-merging derived output. Regenerated changes still deserve review, and a reliable regeneration path is part of maintaining the feature.

Platform-specific build choices sometimes fit naturally in an existing target section, installation script, or fork release workflow. That can avoid spreading the same adjustment across shared files. But toolchain, dependency, and packaging changes can have cross-platform effects: moving a setting out of sight is not an improvement if it changes which builds receive it. Working Android support is worth a necessary shared-file patch.

## Leave useful context, without creating a maintenance ritual

A focused commit and a short explanation of a non-obvious divergence can save more future effort than elaborate isolation. The helpful context is why the behavior is needed, what must survive a merge, and, for a workaround, what would make it removable. Routine code does not need a fork annotation on every line.

Sharing a generally useful fix upstream may eventually eliminate a patch, subject to upstream's contribution process. That is an opportunity, not a prerequisite for improving the fork. Likewise, different branch strategies or merge automation are options for recurring pain, not obligations created by this guide. Automatically keeping one side can hide lost changes rather than reduce the underlying friction.

The aim is less recurring effort for a codebase that remains pleasant to work on. Take the inexpensive opportunities as they arise. When a merge-friendly arrangement makes the code less clear or the behavior less sound, the straightforward implementation wins.

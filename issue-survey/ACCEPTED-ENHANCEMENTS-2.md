# Closed enhancement survey, shard 2: 2026-03-01 through 2026-05-31

## Scope and method

This shard is a sampled review of closed `openai/codex` issues carrying the
`enhancement` label. It covers closure dates from 2026-03-01 through 2026-05-31
inclusive. The outcome tiers below are based on issue/PR evidence, not GitHub's
state reason alone.

I deeply screened **96 unique issues**: 48 discovered with
`reason:completed` and 48 with `reason:"not planned"`. I divided the date range
into six half-month strata and selected eight issues from each closure-reason
pool in each stratum. For each query, I requested up to 100 candidates with
`gh issue list`, sorted the returned candidates by `closedAt`, and selected the
eight evenly spaced ranks
`floor((pool_size - 1) * i / 7), i = 0..7`. The returned pool sizes were:

| Closed-date stratum | `COMPLETED` candidates returned | `NOT_PLANNED` candidates returned | Reviewed |
|---|---:|---:|---:|
| 2026-03-01..03-15 | 37 | 28 | 16 |
| 2026-03-16..03-31 | 48 | 100 (query cap) | 16 |
| 2026-04-01..04-15 | 81 | 49 | 16 |
| 2026-04-16..04-30 | 80 | 41 | 16 |
| 2026-05-01..05-15 | 100 (query cap) | 90 | 16 |
| 2026-05-16..05-31 | 100 (query cap) | 17 | 16 |

For each selected issue I read the title, body, labels, author, close date,
comments, state reason, and closing-PR references. I opened a linked PR when it
determined implementation or proposer-code provenance. The resulting sample
spans App, CLI, TUI, extension, Windows, auth, config, context, subagent, hooks,
MCP, app-server, sandbox, plan, rate-limit, and model-provider work.

This procedure is reproducible and time-stratified, but it is not a random
sample of the complete population. The three 100-result pools may be truncated
and inherit GitHub search's default ranking before my `closedAt` sort.

## Outcome audit

| Evidence tier | Count | Share | What counted here |
|---|---:|---:|---|
| A1 adopted/implemented | 7 | 7.3% | Merged PR or a concrete maintainer statement that the feature was implemented/shipping |
| A2 accepted/committed | 2 | 2.1% | Concrete next-release or roadmap commitment, without an implementation located |
| D1 explicitly declined | 19 | 19.8% | Maintainer supplied a product, architecture, scope, maintenance, or prioritization reason |
| D2 demand-filtered | 35 | 36.5% | Closed specifically for insufficient upvotes/demand |
| X administrative/ambiguous | 33 | 34.4% | Duplicate, withdrawn, malformed, existing behavior, self-closure, or no adoption/decline evidence |
| **Total** | **96** | **100%** | |

The discovery state reason was a poor outcome proxy. Within the 48
`COMPLETED`-selected issues, the audit found 7 A1, 2 A2, 2 D1, 11 D2, and 26 X.
Within the 48 `NOT_PLANNED`-selected issues, it found 17 D1, 24 D2, and 7 X.
In particular, several `COMPLETED` issues were merely demand closures or
author-closed duplicates.

## Substantial code versus ideas

The C/I/U classification concerns what the **issue proposer supplied before
the outcome was decided**, not code later written by OpenAI.

| Qualified outcome set | C substantial code | I idea only | U unclear | Qualified C:I ratio |
|---|---:|---:|---:|---:|
| Adopted (A1 + A2) | 1 | 8 | 0 | **1:8** (11.1% code, 88.9% ideas) |
| Explicitly declined (D1) | 3 | 15 | 1 | **1:5** (16.7% code, 83.3% ideas among known C/I) |
| **Combined evidence-qualified** | **4** | **23** | **1** | **4:23** (14.8% code, 85.2% ideas among known C/I) |

Thus, in this shard, roughly **one in seven classifiable evidence-qualified
suggestions came with a substantial proposer implementation; six in seven
arrived as ideas**. One additional qualified proposal is U because its reported
fork/commit is no longer accessible.
The adopted set was even more idea-heavy. This is not evidence that code hurts
a proposal: the sample is small, the repository generally does not accept
external PRs, and the outcome may be driven by scope, demand, or invariants.

Two additional verified code-bearing proposals,
[#10771](https://github.com/openai/codex/issues/10771) and
[#12945](https://github.com/openai/codex/issues/12945), were D2 demand closures.
[#16726](https://github.com/openai/codex/issues/16726) reported a substantial
implementation but remained X because no adoption or decline evidence
accompanied its `COMPLETED` closure; its linked experimental branches are now
inaccessible, so it is U rather than C. All three are excluded from the
qualified ratio exactly as the rubric requires.
Across all 96 screened issues, including non-qualified outcomes, the C/I/U
counts were 6/88/2.

## Adopted patterns supported by this sample

These categories overlap. They describe recurring properties of the accepted
change, not mutually exclusive product areas.

### 1. Make already-available operational state visible at the point of use

Small visibility gaps were adopted when Codex already possessed the state and
the request exposed it in an established surface: fast-mode state in the TUI
([#14159](https://github.com/openai/codex/issues/14159), A1) and remaining
usage through `/statusline` ([#10328](https://github.com/openai/codex/issues/10328),
A1). This is stronger evidence for “surface existing state” than for building
new monitoring subsystems.

### 2. Turn hidden failure or safety state into an actionable warning

The sample supports contained diagnostic improvements: precise config/rules
parse errors and linked UI toasts ([#11946](https://github.com/openai/codex/issues/11946),
A1), and warning that trusting a Git subdirectory actually trusts the repository
root ([#18505](https://github.com/openai/codex/issues/18505), A2). Both preserve
the underlying mechanism and clarify a consequential state at the decision
point.

### 3. Fill narrow completeness gaps in established CLI/TUI conventions

OpenAI adopted bounded additions whose behavior was already legible from the
surrounding interface: an explicit `codex update` command
([#9274](https://github.com/openai/codex/issues/9274), A1) and support for
F13-F24 in configurable keymaps after F1-F12 already worked
([#25006](https://github.com/openai/codex/issues/25006), A1). These requests did
not require a new workflow model.

### 4. Close concrete platform and terminal parity gaps without changing the abstraction

Windows hooks were enabled through the existing hook design
([#17478](https://github.com/openai/codex/issues/17478), A1), while higher
function keys extended the existing portable keymap representation
([#25006](https://github.com/openai/codex/issues/25006), A1). The common signal
is a contained compatibility hole with a clear boundary and validation path.

### 5. Add integration capabilities that fit an existing provider or harness boundary

Dynamic bearer-token commands for custom model providers were wired through
the existing auth manager and unauthorized-recovery design
([#15189](https://github.com/openai/codex/issues/15189), A1). Remote-development
work was explicitly committed so the app could connect to a harness running in
a container ([#17544](https://github.com/openai/codex/issues/17544), A2). These
are broader than the preceding ergonomic changes, but both extend an already
chosen boundary rather than inventing a parallel one.

## Explicitly declined patterns supported by this sample

Only D1 issues support these inferences. D2 upvote closures are discussed
separately and do not mean OpenAI rejected the underlying kind of feature.

### 1. Bespoke native workflows when existing composable primitives can express them

Maintainers redirected a hierarchical orchestration UI to existing Codex
primitives and a custom client
([#18557](https://github.com/openai/codex/issues/18557), D1), and an
“orchestrator mode” to skills and subagents
([#24807](https://github.com/openai/codex/issues/24807), D1). A replay viewer
could be an external tool over rollout files
([#21940](https://github.com/openai/codex/issues/21940), D1), and goal images
could be referenced as files or steered in later
([#24967](https://github.com/openai/codex/issues/24967), D1). The inference is
not “no orchestration”; it is a preference for primitives over use-case-specific
commands and modes.

### 2. Exceptions that violate thread, context, or composition invariants

Refreshing `AGENTS.md` after changing cwd was declined because instructions are
thread-initial and overriding them can confuse the model
([#16403](https://github.com/openai/codex/issues/16403), D1). Skill-scoped model
defaults or temporary model overrides were rejected because skills compose
inside a thread ([#16266](https://github.com/openai/codex/issues/16266), D1;
[#22908](https://github.com/openai/codex/issues/22908), D1). Persisting `/side`
or attaching durable top-level threads as subagents conflicted with deliberate
lifecycle/core-harness assumptions
([#23472](https://github.com/openai/codex/issues/23472), D1;
[#23713](https://github.com/openai/codex/issues/23713), D1).

### 3. A proposed mechanism that cannot enforce the requested guarantee at its layer

A direct read/write tool was judged unable to fix model behavior and redundant
with `apply_patch` ([#9842](https://github.com/openai/codex/issues/9842), D1).
Separating file-edit approvals from shell approvals cannot provide the claimed
boundary because shell commands also edit files
([#13062](https://github.com/openai/codex/issues/13062), D1). VS Code cannot
provide the requested extension pop-out window in the proposed way
([#14922](https://github.com/openai/codex/issues/14922), D1), tool calls cannot
be deterministically replayed because they are neither reproducible nor
idempotent ([#21940](https://github.com/openai/codex/issues/21940), D1), and
embedding full images in a goal would repeatedly consume model context
([#24967](https://github.com/openai/codex/issues/24967), D1).

### 4. Reversals of deliberate deprecations, defaults, or product direction

Requests to retain slash-command semantics were redirected to the deliberately
chosen `$skill` convention and skills replacement
([#13893](https://github.com/openai/codex/issues/13893), D1;
[#17796](https://github.com/openai/codex/issues/17796), D1). Maintainers also
declined investment in profiles while considering their deprecation
([#14456](https://github.com/openai/codex/issues/14456), D1), declined reverting
the enterprise/business Fast default ([#19230](https://github.com/openai/codex/issues/19230),
D1), and said there were no plans to roll back the newer profile/config layout
([#25331](https://github.com/openai/codex/issues/25331), D1).

### 5. New permanent modes and policy branches with narrow reach or ongoing product cost

This is the weakest and most mergeable category, but the sample suggests
resistance to multiplying durable modes: specialized orchestration modes are
being reduced rather than added ([#24807](https://github.com/openai/codex/issues/24807),
D1); automatic fallback between ChatGPT subscription auth and an API key would
encourage an account/billing pattern the product intentionally replaces with
credits ([#21017](https://github.com/openai/codex/issues/21017), D1); a richer
public issue-deduplicator policy was declined because the maintainer considered
the current behavior adequate ([#24786](https://github.com/openai/codex/issues/24786),
D1); and the config/profile rollback was not planned
([#25331](https://github.com/openai/codex/issues/25331), D1). In synthesis, this
category may be better merged into categories 1 or 4 unless other shards find a
stronger maintenance-cost pattern.

### Cross-cutting D2 signal: insufficient demonstrated demand

Demand filtering was the single largest outcome tier: 35/96. Examples include
`NO_PROXY` support ([#9346](https://github.com/openai/codex/issues/9346), D2),
background jobs ([#11270](https://github.com/openai/codex/issues/11270), D2),
browser auto-reload ([#18327](https://github.com/openai/codex/issues/18327),
D2), and even complete proposer implementations for a Zed URI opener
([#10771](https://github.com/openai/codex/issues/10771), D2, C) and bounded
resume scrollback ([#12945](https://github.com/openai/codex/issues/12945), D2,
C). These are prioritization outcomes, not evidence that OpenAI substantively
dislikes those feature kinds.

## Evidence table: adopted and explicitly declined issues

Every A1/A2 or D1 issue used above is included. “Code” describes the proposer
submission before the outcome.

| Issue | Tier | Outcome evidence | Primary category | Code |
|---|---|---|---|---|
| [#14159](https://github.com/openai/codex/issues/14159) | A1 | Maintainer said the fast-mode status was in the latest CLI and linked [PR #13670](https://github.com/openai/codex/pull/13670). | status visibility | I |
| [#10328](https://github.com/openai/codex/issues/10328) | A1 | Maintainer said persistent/refreshable usage state was now available through `/statusline`. | status visibility | I |
| [#11946](https://github.com/openai/codex/issues/11946) | A1 | Maintainer described already-shipped CLI ranges/nonzero exit and App/extension toast links for config/rules errors. | actionable diagnostics | I |
| [#15189](https://github.com/openai/codex/issues/15189) | A1 | OpenAI-authored [PR #16288](https://github.com/openai/codex/pull/16288) merged and closed the issue with command-backed provider auth. | provider interoperability | I |
| [#17478](https://github.com/openai/codex/issues/17478) | A1 | OpenAI maintainer said Windows hooks were enabled in [PR #17268](https://github.com/openai/codex/pull/17268). | platform parity | I |
| [#9274](https://github.com/openai/codex/issues/9274) | A1 | Maintainer said, “I've added `codex update`” and that it would ship in the next release. | bounded CLI ergonomics | I |
| [#25006](https://github.com/openai/codex/issues/25006) | A1 | OpenAI-authored [PR #25329](https://github.com/openai/codex/pull/25329) merged, extending stored keymaps through F24. | interface completeness | I |
| [#17544](https://github.com/openai/codex/issues/17544) | A2 | Maintainer said remote-development functionality was being built and linked [#10450](https://github.com/openai/codex/issues/10450); no implementation was located for this audit. | harness interoperability | I |
| [#18505](https://github.com/openai/codex/issues/18505) | A2 | Maintainer said the trust-root warning would be addressed in the next release; no merged OpenAI implementation was located. | safety visibility | **C** — proposer supplied a tested [fork PR](https://github.com/joycebeatriz/codex/pull/1) before commitment |
| [#14922](https://github.com/openai/codex/issues/14922) | D1 | Maintainer cited VS Code extension-window limitations and pointed to the desktop app designed around those limits. | wrong layer/host boundary | I |
| [#9842](https://github.com/openai/codex/issues/9842) | D1 | Maintainer said `apply_patch` already provides the safe cross-platform path and direct file APIs would not solve the model behavior. | wrong layer | I |
| [#16266](https://github.com/openai/codex/issues/16266) | D1 | Maintainer said tying composable skills to models/defaults is the wrong abstraction and suggested per-model subagents. | composition invariant | I |
| [#13893](https://github.com/openai/codex/issues/13893) | D1 | Maintainer said custom slash commands were deliberately deprecated in favor of skills. | product direction | I |
| [#16403](https://github.com/openai/codex/issues/16403) | D1 | Maintainer said instructions are sent at thread start; cwd or instruction changes should start a new thread rather than rewrite model context. | context invariant | **U** — proposer reported a tested fork/commit before the decision, but the linked branch and commits are now inaccessible |
| [#13062](https://github.com/openai/codex/issues/13062) | D1 | Maintainer agreed with the problem but rejected file-vs-shell approval separation because shell commands also write files. | ineffective boundary | **C** — proposer supplied a working [fork branch](https://github.com/jarrod-lowe/codex/tree/file-write-policy) |
| [#17796](https://github.com/openai/codex/issues/17796) | D1 | Maintainer pointed to `$` skill invocation and the deliberate removal of `/` custom prompts. | product direction | I |
| [#14456](https://github.com/openai/codex/issues/14456) | D1 | Maintainer said profiles might be deprecated, are little-used, and were low priority for investment. | deprecation direction | **C** — proposer supplied a fix and regression test in a [fork branch](https://github.com/borisroman/codex/tree/fix/config) |
| [#18557](https://github.com/openai/codex/issues/18557) | D1 | Maintainer said all required primitives existed and the use-case-specific UI should be built externally. | existing primitives | I |
| [#19230](https://github.com/openai/codex/issues/19230) | D1 | Maintainer said Fast-by-default for eligible business/enterprise plans was intentional and users could opt out. | intended default | **C** — proposer supplied a clean [revert branch](https://github.com/nwparker/codex/tree/revert-19053-fast-default-enterprise) |
| [#24807](https://github.com/openai/codex/issues/24807) | D1 | Maintainer said a bespoke orchestrator mode was unlikely because specialized modes are being removed; skills/subagents can build the workflow. | surface proliferation | I |
| [#21017](https://github.com/openai/codex/issues/21017) | D1 | Maintainer said subscription/API-key switching is not recommended and account credits are the supported overflow path. | account/product boundary | I |
| [#21940](https://github.com/openai/codex/issues/21940) | D1 | Maintainer said tool calls are neither reproducible nor idempotent; alternate rollout inspection belongs in an external tool. | infeasible mechanism | I |
| [#22908](https://github.com/openai/codex/issues/22908) | D1 | Maintainer said skills compose in a thread and therefore must use its model/effort; a differently configured subagent is the intended boundary. | composition invariant | I |
| [#23472](https://github.com/openai/codex/issues/23472) | D1 | Maintainer said `/side` is intentionally ephemeral and `/fork` or `/new` should be used for persistence. | lifecycle invariant | I |
| [#23713](https://github.com/openai/codex/issues/23713) | D1 | Maintainer said persistent attached top-level sessions would break intrinsic Codex core-harness conceptual/design assumptions. | lifecycle invariant | I |
| [#24786](https://github.com/openai/codex/issues/24786) | D1 | Maintainer explicitly said the current issue-deduplicator behavior was satisfactory. | maintenance/surface choice | I |
| [#24967](https://github.com/openai/codex/issues/24967) | D1 | Maintainer cited per-turn objective resubmission and image context cost, pointing to file references or steering instead. | context boundary | I |
| [#25331](https://github.com/openai/codex/issues/25331) | D1 | Maintainer said there were no plans to revert the newer profile/config direction. | product direction | I |

## Compact audit ledger

This ledger makes the 96-count audit reproducible without using administrative
closures as preference evidence. The parenthesized code is outcome tier and
C/I/U classification.

- March 1-15, `COMPLETED`: [#13176](https://github.com/openai/codex/issues/13176) X/I; [#13371](https://github.com/openai/codex/issues/13371) X/I; [#13528](https://github.com/openai/codex/issues/13528) X/I; [#13863](https://github.com/openai/codex/issues/13863) X/I; [#13994](https://github.com/openai/codex/issues/13994) X/I; [#14159](https://github.com/openai/codex/issues/14159) A1/I; [#14336](https://github.com/openai/codex/issues/14336) X/I; [#10328](https://github.com/openai/codex/issues/10328) A1/I.
- March 16-31, `COMPLETED`: [#14720](https://github.com/openai/codex/issues/14720) X/I; [#13387](https://github.com/openai/codex/issues/13387) X/I; [#10626](https://github.com/openai/codex/issues/10626) X/I; [#15433](https://github.com/openai/codex/issues/15433) X/I; [#15629](https://github.com/openai/codex/issues/15629) X/I; [#15571](https://github.com/openai/codex/issues/15571) X/I; [#11946](https://github.com/openai/codex/issues/11946) A1/I; [#15189](https://github.com/openai/codex/issues/15189) A1/I.
- March 1-15, `NOT_PLANNED`: [#2522](https://github.com/openai/codex/issues/2522) D2/I; [#13554](https://github.com/openai/codex/issues/13554) X/I; [#9346](https://github.com/openai/codex/issues/9346) D2/I; [#8592](https://github.com/openai/codex/issues/8592) D2/I; [#14584](https://github.com/openai/codex/issues/14584) X/I; [#10188](https://github.com/openai/codex/issues/10188) D2/I; [#10311](https://github.com/openai/codex/issues/10311) D2/I; [#14716](https://github.com/openai/codex/issues/14716) X/I.
- March 16-31, `NOT_PLANNED`: [#14922](https://github.com/openai/codex/issues/14922) D1/I; [#9842](https://github.com/openai/codex/issues/9842) D1/I; [#11092](https://github.com/openai/codex/issues/11092) D2/I; [#11270](https://github.com/openai/codex/issues/11270) D2/I; [#10165](https://github.com/openai/codex/issues/10165) D2/I; [#10771](https://github.com/openai/codex/issues/10771) D2/C; [#12447](https://github.com/openai/codex/issues/12447) D2/I; [#16266](https://github.com/openai/codex/issues/16266) D1/I.
- April 1-15, `COMPLETED`: [#16511](https://github.com/openai/codex/issues/16511) X/I; [#16818](https://github.com/openai/codex/issues/16818) X/I; [#17191](https://github.com/openai/codex/issues/17191) X/I; [#13195](https://github.com/openai/codex/issues/13195) D2/I; [#13714](https://github.com/openai/codex/issues/13714) D2/I; [#13893](https://github.com/openai/codex/issues/13893) D1/I; [#17544](https://github.com/openai/codex/issues/17544) A2/I; [#17478](https://github.com/openai/codex/issues/17478) A1/I.
- April 16-30, `COMPLETED`: [#18072](https://github.com/openai/codex/issues/18072) X/I; [#16967](https://github.com/openai/codex/issues/16967) X/I; [#14040](https://github.com/openai/codex/issues/14040) D2/I; [#18505](https://github.com/openai/codex/issues/18505) A2/C; [#19061](https://github.com/openai/codex/issues/19061) X/I; [#16726](https://github.com/openai/codex/issues/16726) X/U; [#9274](https://github.com/openai/codex/issues/9274) A1/I; [#20491](https://github.com/openai/codex/issues/20491) X/I.
- April 1-15, `NOT_PLANNED`: [#16403](https://github.com/openai/codex/issues/16403) D1/U; [#12924](https://github.com/openai/codex/issues/12924) D2/I; [#12945](https://github.com/openai/codex/issues/12945) D2/C; [#12897](https://github.com/openai/codex/issues/12897) D2/I; [#13062](https://github.com/openai/codex/issues/13062) D1/C; [#11921](https://github.com/openai/codex/issues/11921) D2/I; [#13180](https://github.com/openai/codex/issues/13180) D2/I; [#17796](https://github.com/openai/codex/issues/17796) D1/I.
- April 16-30, `NOT_PLANNED`: [#13922](https://github.com/openai/codex/issues/13922) D2/I; [#14195](https://github.com/openai/codex/issues/14195) D2/I; [#14348](https://github.com/openai/codex/issues/14348) D2/I; [#14456](https://github.com/openai/codex/issues/14456) D1/C; [#14713](https://github.com/openai/codex/issues/14713) D2/I; [#18557](https://github.com/openai/codex/issues/18557) D1/I; [#19230](https://github.com/openai/codex/issues/19230) D1/C; [#20519](https://github.com/openai/codex/issues/20519) X/I.
- May 1-15, `COMPLETED`: [#20546](https://github.com/openai/codex/issues/20546) X/I; [#21388](https://github.com/openai/codex/issues/21388) X/I; [#20721](https://github.com/openai/codex/issues/20721) X/I; [#16285](https://github.com/openai/codex/issues/16285) D2/I; [#16598](https://github.com/openai/codex/issues/16598) D2/I; [#16094](https://github.com/openai/codex/issues/16094) D2/I; [#17101](https://github.com/openai/codex/issues/17101) D2/I; [#22853](https://github.com/openai/codex/issues/22853) X/I.
- May 16-31, `COMPLETED`: [#19869](https://github.com/openai/codex/issues/19869) X/I; [#24181](https://github.com/openai/codex/issues/24181) X/I; [#24807](https://github.com/openai/codex/issues/24807) D1/I; [#18327](https://github.com/openai/codex/issues/18327) D2/I; [#18683](https://github.com/openai/codex/issues/18683) D2/I; [#19172](https://github.com/openai/codex/issues/19172) D2/I; [#18611](https://github.com/openai/codex/issues/18611) D2/I; [#25006](https://github.com/openai/codex/issues/25006) A1/I.
- May 1-15, `NOT_PLANNED`: [#13925](https://github.com/openai/codex/issues/13925) D2/I; [#14659](https://github.com/openai/codex/issues/14659) D2/I; [#14897](https://github.com/openai/codex/issues/14897) D2/I; [#15161](https://github.com/openai/codex/issues/15161) D2/I; [#15385](https://github.com/openai/codex/issues/15385) D2/I; [#21017](https://github.com/openai/codex/issues/21017) D1/I; [#21940](https://github.com/openai/codex/issues/21940) D1/I; [#22908](https://github.com/openai/codex/issues/22908) D1/I.
- May 16-31, `NOT_PLANNED`: [#23057](https://github.com/openai/codex/issues/23057) X/I; [#23472](https://github.com/openai/codex/issues/23472) D1/I; [#23859](https://github.com/openai/codex/issues/23859) X/I; [#23713](https://github.com/openai/codex/issues/23713) D1/I; [#24729](https://github.com/openai/codex/issues/24729) X/I; [#24786](https://github.com/openai/codex/issues/24786) D1/I; [#24967](https://github.com/openai/codex/issues/24967) D1/I; [#25331](https://github.com/openai/codex/issues/25331) D1/I.

## Counterexamples, ambiguities, and synthesis recommendations

- **Demand closure is not product dislike.** Azure/custom-provider token refresh
  [#2522](https://github.com/openai/codex/issues/2522) was closed D2 for no
  upvotes, while the more general dynamic-provider-auth request
  [#15189](https://github.com/openai/codex/issues/15189) was implemented later
  that month. Different scope and timing prevent a causal claim, but the pair
  strongly argues against treating D2 as a negative category.
- **Code neither guarantees adoption nor explains it.** [#18505](https://github.com/openai/codex/issues/18505)
  arrived with tested code and was committed, but the maintainer explicitly said
  external code is generally more expensive to review than having Codex write
  it internally; the useful contribution was the root-cause analysis. Conversely,
  [#16403](https://github.com/openai/codex/issues/16403) reported a tested
  implementation but was declined because it violated the thread-context model;
  its now-inaccessible links also illustrate why unavailable provenance is U.
- **“Implemented” includes requests already solved elsewhere.** A1 in this
  audit means there is implementation evidence, not that the issue caused it.
  For example, [#14159](https://github.com/openai/codex/issues/14159) and
  [#10328](https://github.com/openai/codex/issues/10328) were closed by pointing
  to recently available behavior.
- **Administrative X must remain visible.** [#16726](https://github.com/openai/codex/issues/16726)
  reported substantial experimental code, but its unexplained `COMPLETED`
  closure is not adoption evidence. Author-closed duplicates, existing-feature
  answers, blank issues, and self-withdrawals likewise supplied no preference
  signal.
- **One lifecycle decline may rest on a partial mismatch.** The body of
  [#23472](https://github.com/openai/codex/issues/23472) described concurrent
  `resume` continuations, while the maintainer answer discussed `/side`. I kept
  it D1 because the closure stated a deliberate ephemeral-versus-persistent
  lifecycle boundary, but its fit to the exact report is lower confidence.
- **The adopted taxonomy should probably preserve a “small completion gap”
  super-category.** Status visibility, diagnostics, CLI conventions, and
  platform parity are separable mechanisms, but all share small scope, an
  existing abstraction, and objective validation. Other shards can determine
  whether five subcategories are stable or should merge.
- **The fifth D1 category is low confidence.** “Avoid new permanent modes and
  policy branches” may collapse into “use existing primitives” and “do not
  reverse product direction.” Retain it only if other date shards show the same
  maintenance/surface-area rationale explicitly.
- **Creation-date/backlog bias:** the sample is stratified by close date, so
  bulk upvote sweeps closed many older issues in the review period. That inflates
  D2 relative to newly filed proposals and means the shard describes closure
  decisions during these months, not only ideas proposed during them.
- **Search-cap bias:** three strata hit the 100-result cap. The even-rank method
  covers the returned closure-date range but cannot correct omissions caused by
  GitHub's pre-cap ranking. A later synthesis should weight this shard as a
  reproducible landscape sample, not an exhaustive census.

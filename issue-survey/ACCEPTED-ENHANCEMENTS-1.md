# Closed enhancement survey: 2025-04-01 through 2026-02-28

Snapshot date: 2026-08-11.

## Scope and method

I deeply audited 110 closed `openai/codex` issues carrying `enhancement`: 55 selected from `reason:completed` and 55 from `reason:"not planned"`. I queried each closed month separately, sorted candidates by close time, and initially took time-spaced selections rather than sorting for reactions or linked PRs. I then made same-month substitutions to cover distinct feature areas and, in the `NOT_PLANNED` pool, to ensure that explicit D1 decisions were represented rather than letting the large batches of upvote-based closures overwhelm the substantive-decision sample. This is therefore a stratified taxonomy sample, not a probability sample and not an estimate of acceptance rates.

The month/reason allocation was:

| Closed month | `COMPLETED` selector | `NOT_PLANNED` selector |
|---|---:|---:|
| 2025-04 | 5 | 0 |
| 2025-05 | 0 | 0 |
| 2025-06 | 0 | 0 |
| 2025-07 | 1 | 0 |
| 2025-08 | 7 | 2 |
| 2025-09 | 7 | 1 |
| 2025-10 | 7 | 4 |
| 2025-11 | 7 | 8 |
| 2025-12 | 7 | 10 |
| 2026-01 | 7 | 15 |
| 2026-02 | 7 | 15 |
| **Total** | **55** | **55** |

There were no matching May or June issues in the candidate queries, and no `NOT_PLANNED` candidates before August. The sample spans CLI, TUI, IDE extension, app, sandbox, context, MCP, auth/custom models, Windows, model behavior, rate limits, and unlabeled early issues. For each selected issue I inspected the body, proposer, labels, close actor/date, up to 100 comments with authors and associations, closing PR references, and cross-referenced PRs. I inspected linked PR authors, merge state, timing, size, and validation when they controlled A1 or C status. Accounts visibly tied to OpenAI (`*-openai`, `*-oai`) and merged `openai/codex` PRs were treated as maintainer evidence; GitHub often reports those accounts merely as `CONTRIBUTOR`.

Monthly searches can return at most 100 results, and several later months hit that cap. Additional title/comment searches located same-month substitution candidates; nonselected hits are not part of the 110-issue audit counts.

## Outcome audit

State reason was a noisy selector. The evidence-based audit is:

| Candidate selector | A1 adopted/implemented | A2 accepted/committed | D1 explicitly declined | D2 demand-filtered | X administrative/ambiguous | Total |
|---|---:|---:|---:|---:|---:|---:|
| `COMPLETED` | 26 | 0 | 2 | 1 | 26 | 55 |
| `NOT_PLANNED` | 1 | 0 | 22 | 25 | 7 | 55 |
| **Total** | **27** | **0** | **24** | **26** | **33** | **110** |

The surprising cells are real. A `COMPLETED` issue can contain an explicit decline or only an administrative close, while [`#5123`](https://github.com/openai/codex/issues/5123) retained `NOT_PLANNED` after a later maintainer comment said the requested feature shipped in 0.128.0.

## What OpenAI tends to adopt

These are bounded tendencies supported by A1 evidence, not promises that every request of the same kind will be accepted.

### 1. Familiar terminal/editor interaction conventions

Small gaps against established interaction muscle memory are attractive when the behavior is literal and contained: file completion, conventional exit/editor bindings, and ordinary readline operations. Examples: [`#113`](https://github.com/openai/codex/issues/113) (A1, proposer-authored merged PR), [`#148`](https://github.com/openai/codex/issues/148) (A1, proposer-authored merged PR), [`#5932`](https://github.com/openai/codex/issues/5932) (A1, reporter confirmed the release fix), [`#2387`](https://github.com/openai/codex/issues/2387) (A1, maintainer named the shipping PR), and [`#5018`](https://github.com/openai/codex/issues/5018) (A1, merged PR and next-release commitment).

### 2. Make existing state visible and actionable

OpenAI repeatedly adopted requests that expose state Codex already has or turn a vague condition into an actionable UI: model validation, update availability, exact limit reset time, unambiguous context state, and a configurable status line. Examples: [`#32`](https://github.com/openai/codex/issues/32) (A1), [`#244`](https://github.com/openai/codex/issues/244) (A1), [`#2513`](https://github.com/openai/codex/issues/2513) (A1), [`#4440`](https://github.com/openai/codex/issues/4440) (A1), and [`#10293`](https://github.com/openai/codex/issues/10293) (A1).

### 3. Standards-based protocol completeness

Protocol work did well when it completed an already-chosen abstraction rather than adding a parallel framework: initial MCP support, custom Streamable HTTP headers, and MCP elicitation. Examples: [`#5`](https://github.com/openai/codex/issues/5) (A1), [`#5180`](https://github.com/openai/codex/issues/5180) (A1, proposer-authored merged PR), and [`#6992`](https://github.com/openai/codex/issues/6992) (A1, linked merged maintainer PR).

### 4. Correctly scoped configuration, session, and skill lifecycle

Adopted requests often make an existing concept obey the scope users expect: fork a conversation, make `resume --last` local to the working directory, layer project config, recognize a standard user skill directory, or reload skills live. Examples: [`#4690`](https://github.com/openai/codex/issues/4690) (A1), [`#8700`](https://github.com/openai/codex/issues/8700) (A1), [`#2554`](https://github.com/openai/codex/issues/2554) (A1), [`#10493`](https://github.com/openai/codex/issues/10493) (A1), and [`#11069`](https://github.com/openai/codex/issues/11069) (A1).

### 5. Narrow correctness, safety, and platform-completeness fixes

Requests with a concrete failure mode and a bounded fix were adopted even in sensitive areas: graceful rate-limit handling, Homebrew update correctness, a standard Linux entropy device in the sandbox, revisiting over-broad untrusted-project skill filtering, and default web search after security work. Examples: [`#157`](https://github.com/openai/codex/issues/157) (A1), [`#6253`](https://github.com/openai/codex/issues/6253) (A1), [`#12056`](https://github.com/openai/codex/issues/12056) (A1), [`#9696`](https://github.com/openai/codex/issues/9696) (A1), and [`#3139`](https://github.com/openai/codex/issues/3139) (A1).

## What OpenAI tends not to take on

Only D1 issues support these categories. The separate D2 pattern below is a demand signal, not substantive dislike of the feature kind.

### 1. A redundant control when an existing control or the intended API already serves the use case

Maintainers explicitly steered users toward Ctrl+C rather than `/interrupt`, auto-compaction rather than queued manual compaction, app-server steering rather than TUI injection, Codex's own MCP configuration rather than Copilot's, and `resume --all` rather than weakening cwd scoping. Examples: [`#8072`](https://github.com/openai/codex/issues/8072) (D1), [`#3369`](https://github.com/openai/codex/issues/3369) (D1), [`#11415`](https://github.com/openai/codex/issues/11415) (D1), [`#2901`](https://github.com/openai/codex/issues/2901) (D1), and [`#10936`](https://github.com/openai/codex/issues/10936) (D1).

### 2. Investment in a surface or protocol that is being replaced or deprecated

OpenAI declined more work on the outgoing IDE diff view, custom prompts, and legacy MCP SSE transport, pointing to the built-in diff view, skills, and Streamable HTTP/stdio respectively. Examples: [`#6314`](https://github.com/openai/codex/issues/6314) (D1), [`#4734`](https://github.com/openai/codex/issues/4734) (D1), [`#8103`](https://github.com/openai/codex/issues/8103) (D1), [`#9848`](https://github.com/openai/codex/issues/9848) (D1), and [`#2129`](https://github.com/openai/codex/issues/2129) (D1).

### 3. Manual-code-authoring features that conflict with the autonomous-agent direction

The strongest product-direction language in this sample rejected tab completion and ghost-text/inline suggestions as manual-coding-era concepts, directing users toward autonomous Codex or existing completion products. Examples: [`#11761`](https://github.com/openai/codex/issues/11761) (D1) and [`#11898`](https://github.com/openai/codex/issues/11898) (D1).

### 4. Exceptions that strain a safety, lifecycle, or host-surface invariant

Maintainers rejected read-deny sandboxing because of pervasive tool reads, a larger effective context window because it increases terminal errors, IDE worktrees because VS Code cannot switch workspaces per conversation cleanly, raster previews as a poor TUI fit, and process-tree memory governance as outside the agent harness. Examples: [`#7657`](https://github.com/openai/codex/issues/7657) (D1), [`#9429`](https://github.com/openai/codex/issues/9429) (D1), [`#12501`](https://github.com/openai/codex/issues/12501) (D1), [`#11195`](https://github.com/openai/codex/issues/11195) (D1), and [`#11523`](https://github.com/openai/codex/issues/11523) (D1).

### 5. Expanding ownership beyond Codex's support boundary or for one unshared workflow

OpenAI declined a contributor-only Cargo-cache workaround despite supplied code, an updater progress UI controlled by package managers, opening the proprietary app, a new Discord community, and GitHub Copilot subscription login that the Codex team cannot provide. Examples: [`#9397`](https://github.com/openai/codex/issues/9397) (D1), [`#8948`](https://github.com/openai/codex/issues/8948) (D1), [`#10733`](https://github.com/openai/codex/issues/10733) (D1), [`#3662`](https://github.com/openai/codex/issues/3662) (D1), and [`#8361`](https://github.com/openai/codex/issues/8361) (D1).

### Demand filtering is separate

Twenty-six sampled issues were D2: they were closed specifically for insufficient upvotes or follow-on demand. They range from shell explanation ([`#1398`](https://github.com/openai/codex/issues/1398)) and email privacy ([`#2645`](https://github.com/openai/codex/issues/2645)) to reasoning usage output ([`#5276`](https://github.com/openai/codex/issues/5276)), RTL support ([`#5827`](https://github.com/openai/codex/issues/5827)), and a notify-payload field ([`#9657`](https://github.com/openai/codex/issues/9657)). This is a cross-cutting prioritization policy, not evidence that OpenAI dislikes those kinds. [`#5123`](https://github.com/openai/codex/issues/5123) is the decisive counterexample: it was closed for low demand, then implemented later.

## Substantial code versus idea only

The classification concerns what the issue proposer supplied before the relevant outcome, not whether someone else later wrote code.

| Outcome set | C substantial code | I idea only | U unclear | C:I ratio | Code share among C+I |
|---|---:|---:|---:|---:|---:|
| Adopted (A1/A2) | 3 | 24 | 0 | 3:24 = **1:8** | 11.1% |
| Explicitly declined (D1) | 1 | 23 | 0 | **1:23** | 4.2% |
| **Combined qualified** | **4** | **47** | **0** | **4:47** (about 1:11.8) | **7.8%** |

The four C cases were proposer-authored implementations in [`#113`](https://github.com/openai/codex/issues/113), [`#148`](https://github.com/openai/codex/issues/148), [`#5180`](https://github.com/openai/codex/issues/5180), and the declined [`#9397`](https://github.com/openai/codex/issues/9397). “Interested in implementing,” a config/API sketch, a tiny value-only patch, or a PR supplied by another commenter remained I. For example, [`#9429`](https://github.com/openai/codex/issues/9429) included a small repeated constant change and a week of subjective use, but not a substantial implementation of the error-safe context lifecycle being proposed; [`#8640`](https://github.com/openai/codex/issues/8640) linked a working VM library but explicitly said the requested MCP server still needed to be built.

All 26 D2 issues were I under the proposer-specific rubric, so including demand-filtered outcomes would produce 4:73. I exclude D2 from the primary combined ratio because D2 is not substantive acceptance or rejection. The accepted/declined difference is suggestive but far too small and purposively sampled to show that code caused acceptance.

## Evidence tables

### Adopted category evidence

| Issue | Outcome evidence | Category | C/I/U |
|---|---|---|---|
| [`#113`](https://github.com/openai/codex/issues/113) | Proposer authored merged [PR #279](https://github.com/openai/codex/pull/279) | Interaction conventions | C |
| [`#148`](https://github.com/openai/codex/issues/148) | Proposer authored merged [PR #160](https://github.com/openai/codex/pull/160) | Interaction conventions | C |
| [`#5932`](https://github.com/openai/codex/issues/5932) | Reporter [confirmed fixed in 0.55.0](https://github.com/openai/codex/issues/5932#issuecomment-3502804156) | Interaction conventions | I |
| [`#2387`](https://github.com/openai/codex/issues/2387) | Maintainer said it was [added in PR #7606 and would ship next](https://github.com/openai/codex/issues/2387#issuecomment-3684610880) | Interaction conventions | I |
| [`#5018`](https://github.com/openai/codex/issues/5018) | Merged [PR #12455](https://github.com/openai/codex/pull/12455); maintainer said next release | Interaction conventions | I |
| [`#32`](https://github.com/openai/codex/issues/32) | Merged closing [PR #594](https://github.com/openai/codex/pull/594) | Visible/actionable state | I |
| [`#244`](https://github.com/openai/codex/issues/244) | Merged closing [PR #333](https://github.com/openai/codex/pull/333) | Visible/actionable state | I |
| [`#2513`](https://github.com/openai/codex/issues/2513) | Maintainer: [“This is now implemented”](https://github.com/openai/codex/issues/2513#issuecomment-3250346463) | Visible/actionable state | I |
| [`#4440`](https://github.com/openai/codex/issues/4440) | Maintainer said it [had been improved](https://github.com/openai/codex/issues/4440#issuecomment-3519730157) | Visible/actionable state | I |
| [`#10293`](https://github.com/openai/codex/issues/10293) | Maintainer said the status line [would ship next](https://github.com/openai/codex/issues/10293#issuecomment-3854908440) | Visible/actionable state | I |
| [`#5`](https://github.com/openai/codex/issues/5) | Maintainer closed with [live MCP configuration docs](https://github.com/openai/codex/issues/5#issuecomment-3085081660) | Protocol completeness | I |
| [`#5180`](https://github.com/openai/codex/issues/5180) | Proposer authored merged closing [PR #5241](https://github.com/openai/codex/pull/5241) | Protocol completeness | C |
| [`#6992`](https://github.com/openai/codex/issues/6992) | Maintainer linked merged [PR #6947](https://github.com/openai/codex/pull/6947) | Protocol completeness | I |
| [`#4690`](https://github.com/openai/codex/issues/4690) | Maintainer: [CLI now supports `/fork`](https://github.com/openai/codex/issues/4690#issuecomment-3747561765) | Scoped lifecycle | I |
| [`#8700`](https://github.com/openai/codex/issues/8700) | Merged [PR #9245](https://github.com/openai/codex/pull/9245); maintainer named 0.85.0 | Scoped lifecycle | I |
| [`#2554`](https://github.com/openai/codex/issues/2554) | Maintainer said CLI was implemented and IDE was [in the next release](https://github.com/openai/codex/issues/2554#issuecomment-3798258182) | Scoped lifecycle | I |
| [`#10493`](https://github.com/openai/codex/issues/10493) | Maintainer: [included in next release](https://github.com/openai/codex/issues/10493#issuecomment-3842274327) | Scoped lifecycle | I |
| [`#11069`](https://github.com/openai/codex/issues/11069) | Maintainer linked live-update [PR #10478](https://github.com/openai/codex/pull/10478) | Scoped lifecycle | I |
| [`#157`](https://github.com/openai/codex/issues/157) | Maintainer [identified the fixed release](https://github.com/openai/codex/issues/157#issuecomment-2837330184) | Bounded completeness | I |
| [`#6253`](https://github.com/openai/codex/issues/6253) | Maintainer [said fixed and linked the implementation thread](https://github.com/openai/codex/issues/6253#issuecomment-3537918283) | Bounded completeness | I |
| [`#12056`](https://github.com/openai/codex/issues/12056) | Merged closing [PR #12081](https://github.com/openai/codex/pull/12081) | Bounded completeness | I |
| [`#9696`](https://github.com/openai/codex/issues/9696) | Maintainer committed the [risk-based reversal for the next version](https://github.com/openai/codex/issues/9696#issuecomment-3786239975) | Bounded completeness | I |
| [`#3139`](https://github.com/openai/codex/issues/3139) | Maintainer linked changelog and said [web search was default](https://github.com/openai/codex/issues/3139#issuecomment-3825107162) | Bounded completeness | I |

Four additional A1s counted in the audit but not used as taxonomy anchors were [`#110`](https://github.com/openai/codex/issues/110) (I; merged PR), [`#60`](https://github.com/openai/codex/issues/60) (I; linked merged PR addressed a narrower error path), [`#2011`](https://github.com/openai/codex/issues/2011) (I; maintainer said supported), and [`#5123`](https://github.com/openai/codex/issues/5123) (I; implemented after its D2 closure).

### Declined category evidence

| Issue | Outcome evidence | Category | C/I/U |
|---|---|---|---|
| [`#8072`](https://github.com/openai/codex/issues/8072) | Maintainer called Ctrl+C intended and `/interrupt` [redundant](https://github.com/openai/codex/issues/8072#issuecomment-3671044864) | Existing mechanism | I |
| [`#3369`](https://github.com/openai/codex/issues/3369) | Maintainer said reliable auto-compaction should [eliminate manual management](https://github.com/openai/codex/issues/3369#issuecomment-3745437395) | Existing mechanism | I |
| [`#11415`](https://github.com/openai/codex/issues/11415) | Maintainer directed long-lived steering automation to [app server](https://github.com/openai/codex/issues/11415#issuecomment-3888102453) | Existing mechanism | I |
| [`#2901`](https://github.com/openai/codex/issues/2901) | Maintainer kept Codex's common config rather than [Copilot's separate mechanism](https://github.com/openai/codex/issues/2901#issuecomment-3831797556) | Existing mechanism | I |
| [`#10936`](https://github.com/openai/codex/issues/10936) | Maintainer said cwd scoping was [by design and pointed to `--all`](https://github.com/openai/codex/issues/10936#issuecomment-3862466173) | Existing mechanism | I |
| [`#6314`](https://github.com/openai/codex/issues/6314) | Maintainer would not invest in a diff view [scheduled for replacement](https://github.com/openai/codex/issues/6314#issuecomment-3650018315) | Outgoing surface | I |
| [`#4734`](https://github.com/openai/codex/issues/4734) | Maintainer: [custom prompts are being deprecated for skills](https://github.com/openai/codex/issues/4734#issuecomment-3786166437) | Outgoing surface | I |
| [`#8103`](https://github.com/openai/codex/issues/8103) | Same explicit [custom-prompt deprecation](https://github.com/openai/codex/issues/8103#issuecomment-3786169788) | Outgoing surface | I |
| [`#9848`](https://github.com/openai/codex/issues/9848) | Maintainer rejected maintaining [two overlapping mechanisms](https://github.com/openai/codex/issues/9848#issuecomment-3797133688) | Outgoing surface | I |
| [`#2129`](https://github.com/openai/codex/issues/2129) | Maintainer declined legacy SSE in favor of [Streamable HTTP/stdio](https://github.com/openai/codex/issues/2129#issuecomment-3825447734) | Outgoing surface | I |
| [`#11761`](https://github.com/openai/codex/issues/11761) | Maintainer said Codex is autonomous, not [manual code assistance](https://github.com/openai/codex/issues/11761#issuecomment-3898745714) | Autonomous direction | I |
| [`#11898`](https://github.com/openai/codex/issues/11898) | Maintainer explicitly rejected inline completion as [outside Codex's direction](https://github.com/openai/codex/issues/11898#issuecomment-3909505577) | Autonomous direction | I |
| [`#7657`](https://github.com/openai/codex/issues/7657) | Maintainer described pervasive-read failures and recommended [container/VM isolation](https://github.com/openai/codex/issues/7657#issuecomment-3619161519) | Architecture/safety invariant | I |
| [`#9429`](https://github.com/openai/codex/issues/9429) | Maintainer declined because the change would [increase error conditions](https://github.com/openai/codex/issues/9429#issuecomment-3863598273) | Architecture/safety invariant | I |
| [`#12501`](https://github.com/openai/codex/issues/12501) | Maintainer cited VS Code architecture and recommended [the app](https://github.com/openai/codex/issues/12501#issuecomment-3941402236) | Architecture/safety invariant | I |
| [`#11195`](https://github.com/openai/codex/issues/11195) | Maintainer treated raster display as outside terminal support and recommended [the app](https://github.com/openai/codex/issues/11195#issuecomment-3900447354) | Architecture/safety invariant | I |
| [`#11523`](https://github.com/openai/codex/issues/11523) | Maintainer said workload OOM governance [cannot be solved in the harness](https://github.com/openai/codex/issues/11523#issuecomment-3892351497) | Architecture/safety invariant | I |
| [`#9397`](https://github.com/openai/codex/issues/9397) | Maintainer passed on proposer-authored [PR #9399](https://github.com/openai/codex/pull/9399) because only the proposer saw the problem | Ownership/reach boundary | C |
| [`#8948`](https://github.com/openai/codex/issues/8948) | Maintainer said npm/Homebrew control [update progress](https://github.com/openai/codex/issues/8948#issuecomment-3726672598) | Ownership/reach boundary | I |
| [`#10733`](https://github.com/openai/codex/issues/10733) | Maintainer: [no plans to open source the app](https://github.com/openai/codex/issues/10733#issuecomment-3854408866) | Ownership/reach boundary | I |
| [`#3662`](https://github.com/openai/codex/issues/3662) | Maintainer declined Discord and pointed to [GitHub Discussions](https://github.com/openai/codex/issues/3662#issuecomment-3619650719) | Ownership/reach boundary | I |
| [`#8361`](https://github.com/openai/codex/issues/8361) | Maintainer explained Copilot hosting is outside what [Codex can offer](https://github.com/openai/codex/issues/8361#issuecomment-3977309083) | Ownership/reach boundary | I |

Two additional D1s counted but not used as anchors were [`#6315`](https://github.com/openai/codex/issues/6315) (I; redirected to model-training feedback despite a shell-invocation request) and [`#13102`](https://github.com/openai/codex/issues/13102) (I; another custom-prompt deprecation decision).

## Counterexamples, ambiguities, and synthesis recommendations

- **Demand closure is reversible.** [`#5123`](https://github.com/openai/codex/issues/5123) moved from an explicit low-demand closure to implementation without its `NOT_PLANNED` state reason changing. [`#5018`](https://github.com/openai/codex/issues/5018) likewise accumulated duplicate/low-priority signals before a later merged implementation. Later synthesis should never equate D2 with durable rejection.
- **OpenAI sometimes reverses safety/default decisions.** [`#9696`](https://github.com/openai/codex/issues/9696) reversed untrusted-project skill filtering after more risk analysis, and [`#3139`](https://github.com/openai/codex/issues/3139) enabled web search by default after explicitly working through security concerns. “No reversals” is too strong; “reversals need new risk or usage evidence” fits better.
- **Administrative closes were common.** [`#2292`](https://github.com/openai/codex/issues/2292) was closed to track another issue even though a later linked PR implemented WSL image paste; it remains X here. [`#2444`](https://github.com/openai/codex/issues/2444) requested a capability that already existed and produced a documentation fix, so it is also X rather than evidence that the requested feature was adopted.
- **Existing behavior is not preference evidence under this rubric.** Issues answered by `/review`, config, Full Access, an IDE command, or an existing editor setting were X unless the evidence showed a new implementation. That exclusion makes the declined taxonomy narrower but more defensible.
- **Some maintainer responses do not cleanly match the request.** [`#6315`](https://github.com/openai/codex/issues/6315) asked for `pwsh -NoProfile`, but the close redirected it to model-training feedback. I counted the explicit harness/model boundary as D1 but did not use it to define a category. [`#60`](https://github.com/openai/codex/issues/60) had a merged cross-referenced PR addressing a narrower error path, so it is A1 but not a taxonomy anchor.
- **The sample is deliberately D1-enriched.** Later `NOT_PLANNED` months were dominated by standardized upvote closures. Same-month substitutions were necessary to obtain substantive reasons, so the 24 D1 versus 26 D2 balance must not be read as a repository-wide prevalence estimate.
- **Feature-area labels are incomplete.** Many early issues were unlabeled beyond `enhancement`; title/body review supplied the area stratification. Search's 100-result cap also means the later-month discovery frame was not exhaustive.
- **Recommended synthesis taxonomy change:** keep “outgoing/deprecated surfaces” distinct from “redundant existing mechanisms,” and split “architecture” from “external ownership/support boundary.” The evidence for “manual coding versus autonomous agent” is unusually explicit but rests on only two closely related February issues, so retain it with a small-sample warning.

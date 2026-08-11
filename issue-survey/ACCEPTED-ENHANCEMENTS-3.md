# Closed enhancement survey 3: 2026-06-01 through 2026-08-11

## Scope and method

This is a 120-issue landscape sample, not an audit of every issue closed in the
period. I screened 60 `reason:completed` and 60 `reason:"not planned"` issues,
all carrying the current `enhancement` label.

I fetched two authenticated `gh issue list` candidate pools with bodies,
comments, authors, current labels, state reasons, close dates, and closing-PR
references. Both searches reached the requested 100-result cap. Within each
state-reason/month cell, I sorted by the first non-`enhancement` feature label,
close time, and issue number, then took systematic picks across that order. An
initial 50+50 screen was expanded the same way over the remaining candidates to
the 60+60 ceiling.

The resulting month distribution was:

| Closed month | `COMPLETED` | `NOT_PLANNED` | Total |
|---|---:|---:|---:|
| 2026-06 | 18 | 14 | 32 |
| 2026-07 | 31 | 39 | 70 |
| 2026-08 (through Aug 11) | 11 | 7 | 18 |
| **Total** | **60** | **60** | **120** |

The sample spans app (49 issues), CLI (42), TUI (27), session (18), config
(14), app-server (9), subagent (9), extension (7), Windows (7), context (6),
rate limits (6), and smaller provider, browser, skills, MCP, tool-call, Imagen,
plan, and model-behavior strata. Labels overlap, so those counts do not sum to
120.

For every sampled issue I read the body and full fetched comment set. I treated
state reason only as discovery metadata. I inspected the linked merged PRs for
[#27831](https://github.com/openai/codex/issues/27831),
[#26423](https://github.com/openai/codex/issues/26423), and
[#29618](https://github.com/openai/codex/issues/29618) to verify merger and
proposer/implementer identity. The C/I/U assessment asks what the **issue
proposer** supplied before disposition, not whether somebody later wrote code.

## Outcome audit

| Candidate pool | A1 adopted/implemented | A2 accepted/committed | D1 explicitly declined | D2 demand-filtered | X administrative/ambiguous | Total |
|---|---:|---:|---:|---:|---:|---:|
| `COMPLETED` | 13 | 0 | 0 | 0 | 47 | 60 |
| `NOT_PLANNED` | 0 | 0 | 11 | 31 | 18 | 60 |
| **Total** | **13** | **0** | **11** | **31** | **65** | **120** |

The large X count is material: duplicates, withdrawals, issues answered by
existing behavior, reporter self-closures, wrong-repository filings, and
unexplained closures dominated the `COMPLETED` half. `NOT_PLANNED` also
contained withdrawals, duplicates, and unexplained closures. None of those
support a preference claim.

There were no A2 cases. Maintainer statements such as “landing in the next
release” were classified A1 under the rubric because they promised shipment and
closure immediately followed.

## Enhancement kinds adopted in this sample

The categories overlap at the edges; they describe recurring decision shapes,
not mutually exclusive product taxonomies.

### 1. Close a bounded platform or surface-completeness gap

OpenAI adopted small additions that made an established workflow work on one
more platform, file kind, or Codex surface without inventing a parallel
architecture.

- [#26423](https://github.com/openai/codex/issues/26423) (A1): make Windows
  `codex app PATH` use the Desktop deep link; merged
  [PR #26500](https://github.com/openai/codex/pull/26500).
- [#33434](https://github.com/openai/codex/issues/33434) (A1): treat CUDA
  extensions as C++ for existing diff highlighting; a maintainer said it would
  be in the next release.
- [#33221](https://github.com/openai/codex/issues/33221) (A1): Desktop voice
  mode; a maintainer closed it by saying realtime voice and microphone controls
  now ship.

### 2. Expose state Codex already has, then make it actionable

Visibility requests did best when the underlying state already existed and the
new work was a bounded presentation or selection surface.

- [#29618](https://github.com/openai/codex/issues/29618) (A1): expose reset
  credit details in a supported surface and picker; merged
  [PR #30488](https://github.com/openai/codex/pull/30488).
- [#31228](https://github.com/openai/codex/issues/31228) (A1): reset status and
  expiry information; the reporter said a new app update satisfied the request.
- [#31082](https://github.com/openai/codex/issues/31082) (A1): manually mark a
  thread unread; the reporter closed with a screenshot of the shipped control.
- [#33123](https://github.com/openai/codex/issues/33123) (A1): unread-only
  sidebar filtering; the reporter posted the new UI and thanked the product
  owner.

### 3. Remove conspicuous UI confusion or privacy friction

Small UI corrections were adopted when the harm was immediate and the remedy
did not require a new subsystem.

- [#30240](https://github.com/openai/codex/issues/30240) (A1): stop displaying
  the account email in ordinary app chrome; a reporter posted a screenshot of
  the fixed UI.
- [#35913](https://github.com/openai/codex/issues/35913) (A1): remove confusing
  rotating composer placeholders; the maintainer cited user feedback and the
  replacement generic placeholder.

### 4. Complete a provider, plugin, or protocol capability end to end

Interop was adopted when it extended an existing abstraction and had a clear
capability boundary rather than adding one vendor-specific side channel.

- [#27831](https://github.com/openai/codex/issues/27831) (A1): accept npm-backed
  marketplace plugin sources; merged
  [PR #29375](https://github.com/openai/codex/pull/29375).
- [#31380](https://github.com/openai/codex/issues/31380) (A1): let the built-in
  Bedrock provider use the header plumbing it already had; the issue links the
  resulting [OpenAI commit](https://github.com/openai/codex/commit/315195492c80fdade38e917c18f9584efd599304).
- [#28912](https://github.com/openai/codex/issues/28912) (A1): make MCP Apps
  render end to end; a maintainer confirmed inline, side-panel, and fullscreen
  support.

### 5. Put repeated runtime policy at an existing configuration choke point

Configuration was favored when it expressed durable policy at a place that
already owned the behavior. This pattern overlaps the interop category.

- [#26767](https://github.com/openai/codex/issues/26767) (A1): global subagent
  model/provider/reasoning defaults; a maintainer said it was landing in the
  next release.
- [#31380](https://github.com/openai/codex/issues/31380) (A1): Bedrock custom
  headers were admitted at provider-config validation, while reusing existing
  request/signing plumbing.
- [#27831](https://github.com/openai/codex/issues/27831) (A1): npm became one
  more validated source kind in the existing marketplace schema and install
  path.

## Enhancement kinds declined in this sample

Only D1 supports the first four substantive patterns below. The fifth is D2
and is deliberately labeled as a demand filter, not evidence that OpenAI
dislikes those feature kinds.

### 1. Prefer skills, plugins, instructions, and explicit invocation over a specialized built-in

When an extensibility surface could express the behavior, maintainers resisted
hard-coding another composer feature or slash command.

- [#26337](https://github.com/openai/codex/issues/26337) (D1): configurable
  prompt snippets; the maintainer directed the reporter to skills.
- [#33186](https://github.com/openai/codex/issues/33186) (D1): a built-in
  `/orchestrator`; the maintainer preferred a skill or plugin.
- [#34565](https://github.com/openai/codex/issues/34565) (D1): authoritative
  external per-turn skill routing; the maintainer pointed to instructions,
  explicit skill invocation, and organization-managed marketplaces within the
  supported ownership boundary.

### 2. Reuse an existing semantic control instead of adding parallel CLI machinery

New flags or lifecycle commands were declined when a prompt, existing key
choice, or current command already represented the intended action.

- [#26966](https://github.com/openai/codex/issues/26966) (D1): `codex exec
  --goal`; the maintainer said to ask the agent to create the goal and requested
  a concrete failure reproduction when that alternative was challenged.
- [#26393](https://github.com/openai/codex/issues/26393) (D1): always bypass the
  queue for `!` commands; Enter already steers while Tab deliberately queues,
  preserving both use cases.
- [#33370](https://github.com/openai/codex/issues/33370) (D1): `/restart`; the
  maintainer found the mechanism insufficiently distinct from `/new` and said
  the command was unlikely as described.

### 3. Preserve intentional interface grammar and curated presentation

Small patches were still declined when they reversed a deliberate convention
rather than filling a missing capability.

- [#25262](https://github.com/openai/codex/issues/25262) (D1): alphabetize slash
  commands; the maintainer retained frequency-curated presentation order.
- [#27983](https://github.com/openai/codex/issues/27983) (D1): treat bare
  `exit`/`quit` as commands; the maintainer retained the slash-prefixed boundary
  between client commands and model prompts.
- [#26393](https://github.com/openai/codex/issues/26393) (D1): immediate shell
  execution would erase the deliberate steer-versus-queue choice.

### 4. Do not reverse deliberate model or product-surface direction

Maintainers were explicit when a request pulled toward a workflow they no
longer considered strategic or a surface they did not plan to support.

- [#25816](https://github.com/openai/codex/issues/25816) (D1): retain old
  models; the maintainer said there were no plans to keep them.
- [#26124](https://github.com/openai/codex/issues/26124) (D1): IDE code
  autocompletion; the maintainer said manual code editing was not a
  forward-looking Codex workflow.
- [#26136](https://github.com/openai/codex/issues/26136) (D1): TUI
  localization; the maintainer contrasted the already localized app with no
  plan to localize the TUI.

### 5. Insufficient demonstrated demand is a cross-cutting filter, not a product taxonomy

Thirty-one issues were closed with the same insufficient-upvote explanation.
They cover unrelated feature kinds, so they show a prioritization mechanism,
not a coherent set of disliked enhancements. Examples include recovery after
failed compaction [#21288](https://github.com/openai/codex/issues/21288) (D2),
composer text selection [#20645](https://github.com/openai/codex/issues/20645)
(D2), full-page browser screenshots
[#20146](https://github.com/openai/codex/issues/20146) (D2), a node-based Imagen
workflow [#21157](https://github.com/openai/codex/issues/21157) (D2), and
project-scoped/lazy MCP startup
[#20494](https://github.com/openai/codex/issues/20494) (D2).

## Proposer code versus idea-only

These counts cover evidence-qualified A1/A2 and D1 outcomes. D2 and X are not
in the denominator. A later OpenAI implementation remains I, and code supplied
by a commenter other than the issue proposer does not make the proposal C.

| Outcome set | C substantial code | I idea only | U unclear | Qualified C:I denominator | C:I ratio | Qualified shares |
|---|---:|---:|---:|---:|---:|---:|
| Adopted (A1/A2) | 1 | 12 | 0 | 13 | **1:12** | 7.7% C / 92.3% I |
| Explicitly declined (D1) | 3 | 6 | 2 | 9 | **1:2** | 33.3% C / 66.7% I |
| **Combined** | **4** | **18** | **2** | **22** | **2:9** | **18.2% C / 81.8% I** |

The sole adopted C case was
[#28912](https://github.com/openai/codex/issues/28912), whose proposer supplied
a fork commit and compare link implementing most of the MCP Apps path. The
three declined C cases were
[#25262](https://github.com/openai/codex/issues/25262),
[#27983](https://github.com/openai/codex/issues/27983), and
[#26966](https://github.com/openai/codex/issues/26966). The two D1 U cases were
[#26136](https://github.com/openai/codex/issues/26136) and
[#33186](https://github.com/openai/codex/issues/33186): both claimed local
implementations, but the issue did not provide an inspectable Codex fork/patch
link sufficient to verify the code.

This sample therefore does **not** support “bring code and OpenAI will take the
feature.” It also does not support the reverse causal claim. It shows only that
most qualified proposals arrived as ideas, while substantial implementations
appeared on both sides of the decision boundary.

## Compact evidence table

The table includes every A1 and D1 issue used in the categories above.

| Issue | Tier and decisive outcome evidence | Primary category | C/I/U |
|---|---|---|---|
| [#26423](https://github.com/openai/codex/issues/26423) | A1 — OpenAI-authored [PR #26500](https://github.com/openai/codex/pull/26500) merged and says it fixes the issue | Surface completeness | I — workaround and design, no Codex implementation |
| [#33434](https://github.com/openai/codex/issues/33434) | A1 — [maintainer promised the next release](https://github.com/openai/codex/issues/33434#issuecomment-5036109586) | Surface completeness | I — one-line direction, not substantial code |
| [#33221](https://github.com/openai/codex/issues/33221) | A1 — [maintainer said Desktop now ships voice](https://github.com/openai/codex/issues/33221#issuecomment-5187340107) | Surface completeness | I |
| [#29618](https://github.com/openai/codex/issues/29618) | A1 — OpenAI-authored [PR #30488](https://github.com/openai/codex/pull/30488) merged and says it fixes the issue | State exposure | I — later prototype was from another commenter |
| [#31228](https://github.com/openai/codex/issues/31228) | A1 — reporter said the new app update satisfied the request | State exposure | I |
| [#31082](https://github.com/openai/codex/issues/31082) | A1 — reporter posted the shipped unread control | State exposure | I |
| [#33123](https://github.com/openai/codex/issues/33123) | A1 — reporter posted and acknowledged the shipped unread-filter UI | State exposure | I |
| [#30240](https://github.com/openai/codex/issues/30240) | A1 — [reporter posted a fixed-UI screenshot](https://github.com/openai/codex/issues/30240#issuecomment-4817903668) | UI clarity/privacy | I |
| [#35913](https://github.com/openai/codex/issues/35913) | A1 — [maintainer described the decision and replacement](https://github.com/openai/codex/issues/35913#issuecomment-5211502745) | UI clarity | I |
| [#27831](https://github.com/openai/codex/issues/27831) | A1 — OpenAI-authored [PR #29375](https://github.com/openai/codex/pull/29375) merged and says it fixes the issue | Interop | I — reproducer/package, no Codex feature code |
| [#31380](https://github.com/openai/codex/issues/31380) | A1 — issue links the resulting [OpenAI commit](https://github.com/openai/codex/commit/315195492c80fdade38e917c18f9584efd599304) | Interop/config | I — source analysis and proposed change only |
| [#28912](https://github.com/openai/codex/issues/28912) | A1 — [maintainer confirmed end-to-end MCP Apps support](https://github.com/openai/codex/issues/28912#issuecomment-5187323645) | Interop | **C** — proposer supplied fork commit and compare |
| [#26767](https://github.com/openai/codex/issues/26767) | A1 — [maintainer said it was landing next release](https://github.com/openai/codex/issues/26767#issuecomment-4991086610) | Runtime policy | I |
| [#26337](https://github.com/openai/codex/issues/26337) | D1 — [maintainer said to use skills](https://github.com/openai/codex/issues/26337#issuecomment-4626962776) | Extensibility over built-in | I |
| [#33186](https://github.com/openai/codex/issues/33186) | D1 — [maintainer preferred a skill/plugin](https://github.com/openai/codex/issues/33186#issuecomment-5030442163) | Extensibility over built-in | **U** — claimed local Codex prototype was not inspectable from the issue |
| [#34565](https://github.com/openai/codex/issues/34565) | D1 — [maintainer said the harness cannot delegate model-owned implicit selection](https://github.com/openai/codex/issues/34565#issuecomment-5035921184) | Extensibility boundary | I |
| [#26966](https://github.com/openai/codex/issues/26966) | D1 — [maintainer said a prompt already creates a goal](https://github.com/openai/codex/issues/26966#issuecomment-4665104723) | Existing semantic control | **C** — proposer supplied compare, implementation, and tests |
| [#26393](https://github.com/openai/codex/issues/26393) | D1 — [maintainer retained both queue and steer semantics](https://github.com/openai/codex/issues/26393#issuecomment-4627090060) | Existing semantic control | I |
| [#33370](https://github.com/openai/codex/issues/33370) | D1 — [maintainer said `/restart` was unlikely as described](https://github.com/openai/codex/issues/33370#issuecomment-5030447311) after asking how it differed from `/new` | Existing semantic control | I |
| [#25262](https://github.com/openai/codex/issues/25262) | D1 — [maintainer retained intentional curated order](https://github.com/openai/codex/issues/25262#issuecomment-4589265365) | Interface convention | **C** — fork branch, commit, and validation supplied |
| [#27983](https://github.com/openai/codex/issues/27983) | D1 — [maintainer retained slash-command grammar](https://github.com/openai/codex/issues/27983#issuecomment-4715307199) | Interface convention | **C** — linked fork branch and focused commit |
| [#25816](https://github.com/openai/codex/issues/25816) | D1 — [maintainer said old models would not be kept](https://github.com/openai/codex/issues/25816#issuecomment-4604452829) | Product direction | I |
| [#26124](https://github.com/openai/codex/issues/26124) | D1 — [maintainer called autocomplete non-forward-looking](https://github.com/openai/codex/issues/26124#issuecomment-4614976400) | Product direction | I |
| [#26136](https://github.com/openai/codex/issues/26136) | D1 — [maintainer said there were no TUI-localization plans](https://github.com/openai/codex/issues/26136#issuecomment-4615000096) | Product-surface direction | **U** — local branch claimed but no inspectable link supplied |

## Counterexamples, ambiguities, and synthesis recommendations

- **Code is not acceptance evidence.** Code-heavy
  [#25262](https://github.com/openai/codex/issues/25262),
  [#27983](https://github.com/openai/codex/issues/27983), and
  [#26966](https://github.com/openai/codex/issues/26966) were substantively
  declined, while 12 of 13 adopted issues were idea-only at proposal time.
- **Code in an administrative closure stays out of preference claims.**
  [#29865](https://github.com/openai/codex/issues/29865) supplied a validated
  branch but self-closed as a duplicate. [#26889](https://github.com/openai/codex/issues/26889)
  carried a large implementation but the author later closed it as “not
  planned” without a maintainer rationale. Both are X here.
- **Provenance matters.** [#20210](https://github.com/openai/codex/issues/20210)
  later received a substantial patch from a different commenter, but the issue
  proposer had brought an idea; this cannot count as proposer C. It was also D2,
  not a substantive rejection.
- **Some A1 evidence is weaker than a linked PR.** The app-only cases
  [#30240](https://github.com/openai/codex/issues/30240),
  [#31082](https://github.com/openai/codex/issues/31082),
  [#31228](https://github.com/openai/codex/issues/31228), and
  [#33123](https://github.com/openai/codex/issues/33123) rely on specific
  reporter-observed shipped UI, sometimes with screenshots. They establish that
  the requested behavior appeared, but not that the issue caused it.
- **The candidate pools were capped.** Each broad `gh` query hit 100 results,
  so systematic stratification improves coverage but does not make this a
  uniform sample of every closure in the date range. Search ordering, current
  labels, and the July batch of demand closures can bias the mix. The many D2
  closures especially should not be extrapolated as product dislike.
- **The date window is short and unusually recent.** It captures a fast-moving
  product period and excludes `reason:duplicate` by assignment. Ratios are
  sample descriptions, not population estimates.
- **Recommended taxonomy change for synthesis:** retain “surface/platform
  completeness,” “existing-state exposure,” “bounded UI friction,”
  “provider/protocol interoperability,” and “configuration at an existing
  choke point” as separate adopted patterns. On the declined side, split
  “there is already a prompt/skill/plugin path” from “this would violate a
  deliberate grammar or ownership boundary,” and always report D2 demand
  filtering outside the substantive disliked-feature taxonomy.

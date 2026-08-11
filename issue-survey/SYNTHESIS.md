# Synthesis: what kinds of Codex enhancements OpenAI adopts

## Executive finding

Across this sample, OpenAI most often adopted **bounded, objectively verifiable
extensions of something Codex already does**. It most often declined **parallel
mechanisms, exceptions to deliberate invariants, and requests aimed away from
the product's chosen direction or ownership boundary**.

The practical distinction is less “which feature area does OpenAI like?” than:

> Does this proposal complete an existing abstraction at its natural ownership
> point, or does it create another abstraction, mode, or support obligation?

The three shards deeply screened 326 closed `enhancement` issues spanning April
2025 through August 11, 2026. GitHub's close reason was treated only as a
candidate selector because both `COMPLETED` and `NOT_PLANNED` contained many
false positives for the outcome they appear to describe.

| Evidence-based outcome | Count | Role in this synthesis |
|---|---:|---|
| A1 adopted/implemented | 47 | Supports adopted categories |
| A2 accepted/committed | 2 | Supports adopted categories, with weaker implementation evidence |
| D1 explicitly declined | 54 | Supports declined categories |
| D2 demand-filtered | 92 | Separate prioritization signal, not product dislike |
| X administrative/ambiguous | 131 | Excluded from preference claims |
| **Total screened** | **326** | Stratified landscape sample, not an acceptance-rate estimate |

## Five enhancement shapes OpenAI tends to adopt

### 1. Close a small completeness or parity gap in an established interface

The most durable pattern is a literal missing case whose surrounding behavior
already defines the answer: conventional terminal/editor actions, an existing
workflow on another platform, another recognized file type, or a missing option
on a sibling command. These requests are bounded, easy to explain, and
objectively testable.

Examples include conventional input behavior
([#113](https://github.com/openai/codex/issues/113),
[#148](https://github.com/openai/codex/issues/148)), F13-F24 in an existing
keymap system ([#25006](https://github.com/openai/codex/issues/25006)), the
Windows `codex app PATH` deep-link path
([#26423](https://github.com/openai/codex/issues/26423)), and recognizing CUDA
files in existing diff highlighting
([#33434](https://github.com/openai/codex/issues/33434)).

The favored shape is “make the current feature complete,” not “introduce a new
mode that partly overlaps it.”

### 2. Surface state Codex already has and make it actionable

Visibility proposals did well when the underlying state already existed and the
new work put it at the user's decision point: status, remaining usage, reset
time, update state, unread state, or a precise warning.

Examples include exact limit-reset visibility
([#2513](https://github.com/openai/codex/issues/2513)), a configurable status
line ([#10293](https://github.com/openai/codex/issues/10293)), visible Fast-mode
state ([#14159](https://github.com/openai/codex/issues/14159)), detailed reset
credits ([#29618](https://github.com/openai/codex/issues/29618)), and unread
thread controls ([#31082](https://github.com/openai/codex/issues/31082),
[#33123](https://github.com/openai/codex/issues/33123)).

This is stronger evidence for exposing existing truth than for adding a new
monitoring or state-management subsystem.

### 3. Complete a chosen protocol, provider, or plugin boundary end to end

Interop was repeatedly adopted when it made an existing standards-based or
capability-based abstraction work fully. The request had a natural place in an
MCP, provider-auth, marketplace, or provider-configuration path and did not
require a vendor-specific side channel.

Examples include MCP Streamable HTTP headers
([#5180](https://github.com/openai/codex/issues/5180)), MCP elicitation
([#6992](https://github.com/openai/codex/issues/6992)), dynamic bearer-token
refresh for custom providers
([#15189](https://github.com/openai/codex/issues/15189)), npm-backed plugin
sources ([#27831](https://github.com/openai/codex/issues/27831)), and Bedrock
headers through existing provider plumbing
([#31380](https://github.com/openai/codex/issues/31380)).

The successful framing is capability completeness at an existing boundary, not
special treatment for one service.

### 4. Put configuration, policy, and lifecycle behavior at the scope that owns it

OpenAI adopted changes that made an existing concept obey a clear scope or
lifecycle: project versus user configuration, current working directory,
session forking/resumption, skill discovery/reload, or global defaults for a
runtime concept.

Examples include `/fork`
([#4690](https://github.com/openai/codex/issues/4690)), cwd-scoped `resume
--last` ([#8700](https://github.com/openai/codex/issues/8700)), project config
layering ([#2554](https://github.com/openai/codex/issues/2554)), live skill
reload ([#11069](https://github.com/openai/codex/issues/11069)), and global
subagent runtime defaults
([#26767](https://github.com/openai/codex/issues/26767)).

These requests expand policy at an existing choke point rather than threading a
one-off switch through unrelated layers.

### 5. Remove concrete UX, correctness, privacy, or safety friction with a bounded fix

Small but consequential friction was adopted when the report named an immediate
failure or confusion and the remedy did not require a new subsystem. This
includes actionable diagnostics and carefully scoped corrections even in
security-sensitive behavior.

Examples include precise config/rules parse errors
([#11946](https://github.com/openai/codex/issues/11946)), a trust-root warning
([#18505](https://github.com/openai/codex/issues/18505), A2), Linux sandbox
access to the standard entropy device
([#12056](https://github.com/openai/codex/issues/12056)), removing account-email
exposure from ordinary app chrome
([#30240](https://github.com/openai/codex/issues/30240)), and replacing confusing
rotating composer placeholders
([#35913](https://github.com/openai/codex/issues/35913)).

The common property is a concrete harm and a contained, reviewable correction.

## Five enhancement shapes OpenAI tends to decline

These categories use only explicit D1 product decisions. Low-upvote D2 closures
are reported separately below.

### 1. A specialized built-in when an existing primitive already expresses the job

Maintainers regularly redirected requests to prompts, skills, plugins, hooks,
app-server, OS controls, or an existing command/key choice. The objection was
not necessarily to the use case; it was to permanently adding a second way to
represent it.

Examples include `/interrupt` versus Ctrl+C
([#8072](https://github.com/openai/codex/issues/8072)), TUI injection versus
app-server steering ([#11415](https://github.com/openai/codex/issues/11415)),
prompt snippets versus skills
([#26337](https://github.com/openai/codex/issues/26337)), `codex exec --goal`
versus asking the agent to create a goal
([#26966](https://github.com/openai/codex/issues/26966)), and a built-in
`/orchestrator` versus a skill or plugin
([#33186](https://github.com/openai/codex/issues/33186)).

### 2. Exceptions that violate a thread, lifecycle, context, safety, or host invariant

Requests were declined when implementing them would require Codex to weaken a
deliberate invariant or introduce special lifecycle semantics: rewriting thread
context, making an intentionally ephemeral object persistent, attaching objects
whose identities have different meanings, or claiming a safety boundary that
other tools can bypass.

Examples include reloading `AGENTS.md` after cwd changes
([#16403](https://github.com/openai/codex/issues/16403)), separating file-edit
approval from shell approval even though shells also edit files
([#13062](https://github.com/openai/codex/issues/13062)), persisting `/side`
conversations ([#23472](https://github.com/openai/codex/issues/23472)), and
attaching durable top-level sessions as subagents
([#23713](https://github.com/openai/codex/issues/23713)).

A working patch did not overcome this category of objection.

### 3. A mechanism at a layer that cannot deliver the promised guarantee

Some requests targeted a real problem but proposed an enforcement point that
could not control it. Maintainers preferred correcting model behavior, using a
different product boundary, or acknowledging that the requested guarantee was
not technically sound.

Examples include adding direct read/write tools to correct model behavior even
though `apply_patch` already exists
([#9842](https://github.com/openai/codex/issues/9842)), deterministic replay of
non-idempotent tool calls
([#21940](https://github.com/openai/codex/issues/21940)), putting full images in
a goal that is resubmitted every turn
([#24967](https://github.com/openai/codex/issues/24967)), authoritative external
skill routing when implicit selection belongs to the model
([#34565](https://github.com/openai/codex/issues/34565)), and process-tree OOM
governance inside the agent harness
([#11523](https://github.com/openai/codex/issues/11523)).

### 4. Reversing a deliberate deprecation, interface grammar, default, or product direction

Small implementation cost did not help when the request pointed backward from a
conscious product decision. Maintainers explicitly defended replacement
mechanisms, curated presentation, client/model grammar, current model policy,
and Codex's shift toward autonomous rather than manual code authoring.

Examples include restoring custom prompts instead of skills
([#4734](https://github.com/openai/codex/issues/4734)), retaining legacy MCP SSE
transport ([#2129](https://github.com/openai/codex/issues/2129)), rolling back
the profile/config direction
([#25331](https://github.com/openai/codex/issues/25331)), treating bare
`exit`/`quit` as client commands rather than prompts
([#27983](https://github.com/openai/codex/issues/27983)), retaining old models
([#25816](https://github.com/openai/codex/issues/25816)), and IDE
autocompletion ([#26124](https://github.com/openai/codex/issues/26124)).

Reversals can happen, but the sample suggests they need new usage, risk, or
security evidence—not merely a patch that restores the old behavior.

### 5. Support obligations outside Codex's ownership or with very narrow reach

Maintainers declined work controlled by another layer or useful to too small a
population to justify permanent support: package-manager behavior, another
product's account model, a host application's limitation, one contributor's
environment, or a niche integration.

Examples include a contributor-specific Cargo-cache workaround despite a PR
([#9397](https://github.com/openai/codex/issues/9397)), updater progress owned by
npm/Homebrew ([#8948](https://github.com/openai/codex/issues/8948)), GitHub
Copilot subscription login
([#8361](https://github.com/openai/codex/issues/8361)), a VS Code window model
the host cannot provide
([#14922](https://github.com/openai/codex/issues/14922)), and opening the
proprietary app source
([#10733](https://github.com/openai/codex/issues/10733)).

## The large sixth signal: demonstrate demand before the issue ages out

Ninety-two of 326 screened issues were closed specifically for insufficient
upvotes or demonstrated demand. That is a prioritization policy, not a sixth
category of features OpenAI dislikes: the affected requests ranged across
shells, accessibility, browser behavior, provider support, background work, and
UI controls.

Demand closure was also reversible. For example,
[#5123](https://github.com/openai/codex/issues/5123) was closed for low demand
and later implemented without its `NOT_PLANNED` reason being updated. Complete
proposer implementations could also be demand-filtered, as happened with a Zed
URI opener ([#10771](https://github.com/openai/codex/issues/10771)) and bounded
resume scrollback ([#12945](https://github.com/openai/codex/issues/12945)).

The practical lesson is that demonstrated breadth of need affects triage
independently of technical quality.

## Does arriving with code help?

Usually, the suggestion arrived as an idea. The proposer-code classification
asks what the issue author supplied before the outcome—not whether OpenAI later
implemented it.

| Evidence-qualified outcome | Substantial proposer code | Idea only | Unclear | C:I among known | Code share among known |
|---|---:|---:|---:|---:|---:|
| Adopted or committed (A1/A2) | 5 | 44 | 0 | **5:44** | **10.2%** |
| Explicitly declined (D1) | 7 | 44 | 3 | **7:44** | **13.7%** |
| **Combined** | **12** | **88** | **3** | **3:22** | **12.0%** |

So, among the 100 evidence-qualified cases with known provenance, about **one in
eight** arrived with a substantial implementation and **seven in eight** arrived
as an idea. Code-bearing requests were not overrepresented among adopted issues
in this sample; their observed share was slightly higher among explicit
declines.

This does **not** show that code hurts a proposal. The samples were stratified
for taxonomy rather than statistical inference, and the decision often turned
on product fit or invariants. It does show that code is neither necessary nor
sufficient:

- 44 adopted requests arrived as ideas.
- Code-backed requests were accepted when they fit an existing abstraction,
  such as [#5180](https://github.com/openai/codex/issues/5180).
- Code-backed requests were declined when they conflicted with intentional
  grammar, lifecycle, or scope, such as
  [#25262](https://github.com/openai/codex/issues/25262),
  [#26966](https://github.com/openai/codex/issues/26966), and
  [#27983](https://github.com/openai/codex/issues/27983).

The best contribution is therefore often a precise root-cause analysis,
bounded design, and objective validation plan. A fork implementation is useful
as feasibility evidence, but it does not substitute for product fit.

## Practical guidance for proposing an enhancement

1. **Frame it as the smallest missing case in an existing abstraction.** Name
   the current command, lifecycle, provider capability, configuration owner, or
   UI state that is incomplete.
2. **Explain why the supported primitive is insufficient.** Check prompts,
   skills, plugins, hooks, app-server, existing commands, and OS controls before
   asking for another built-in path.
3. **Preserve the architecture's ownership boundaries.** Put provider behavior
   in provider capability/configuration, session behavior in session lifecycle,
   and durable policy at an existing configuration choke point.
4. **Lead with a concrete failure and an objective acceptance test.** A narrow
   reproduction, affected surfaces, and expected behavior are more persuasive
   than a broad implementation blueprint.
5. **Show breadth of demand.** Explain who else encounters the problem and why
   existing alternatives do not scale; low engagement is itself a common close
   path.
6. **Treat code as supporting evidence.** If supplying a branch, keep it small,
   validated, and easy to compare, but make the issue stand on its problem and
   product fit without the patch.
7. **For a reversal, bring new evidence.** A request to restore deprecated
   behavior or change an intentional default needs new usage, safety, or
   security information that changes the original tradeoff.

## Confidence and limitations

The most stable cross-period findings are: bounded completion gaps, existing
state visibility, and protocol/provider completeness on the adopted side; and
existing-primitive redundancy, invariant violations, and deliberate product
direction on the declined side.

The sample is not a census or an acceptance-rate study. It was stratified by
close date and state-reason selector, later pools sometimes hit GitHub's
100-result search cap, the early shard was intentionally enriched for explicit
D1 rationales, and some closed-source app implementations were verified through
maintainer or reporter observations rather than linked code. Current labels and
bulk low-upvote closure sweeps introduce additional bias.

Most importantly, implementation evidence proves that requested behavior
appeared; it does not prove that a particular issue caused OpenAI to build it.

## Source shards

- [April 2025 through February 2026](ACCEPTED-ENHANCEMENTS-1.md)
- [March through May 2026](ACCEPTED-ENHANCEMENTS-2.md)
- [June through August 11, 2026](ACCEPTED-ENHANCEMENTS-3.md)
- [Shared evidence rubric and survey instructions](INSTRUCTIONS.md)

# OpenAI Codex closed-enhancement survey instructions

## Research question

Use closed `openai/codex` issues carrying the `enhancement` label to infer:

1. Roughly five kinds of enhancement requests OpenAI tends to adopt.
2. Roughly five kinds OpenAI tends not to take on.
3. Among evidence-qualified requests, the ratio of proposals that arrived with a
   substantial implementation to proposals that arrived as ideas only.

This is a sampled landscape review, not a claim that every closed issue was
manually audited. Keep facts, reasonable inferences, and unknowns distinct.

Snapshot on 2026-08-11: GitHub search reported 2,639 closed enhancement issues:
1,293 `COMPLETED`, 918 `NOT_PLANNED`, and 428 `DUPLICATE`. These state reasons
are candidate selectors, not ground truth about adoption.

## Why state reason alone is insufficient

- `COMPLETED` can include an author-closing a duplicate. For example, #37826
  says "Duplicate of #37825" despite its `COMPLETED` state reason.
- `NOT_PLANNED` includes withdrawn, malformed, filed-in-error, duplicate, and
  no-response issues as well as deliberate product rejections.
- A linked closing PR is strong implementation evidence, but it does not prove
  that the issue reporter supplied code. Inspect issue and PR authors/timing.

Exclude administrative closures from preference/category evidence. Record them
in the audit counts so the exclusions remain visible.

## Outcome evidence rubric

Classify each reviewed issue as one of:

- **A1 adopted/implemented:** merged linked PR, release/fix reference, or a
  maintainer says it is implemented or will ship and closure follows.
- **A2 accepted/committed:** explicit maintainer acceptance or concrete roadmap
  commitment, but no implementation located. Keep separate from A1.
- **D1 explicitly declined:** maintainer gives a product, architecture, scope,
  maintenance, or prioritization reason for not taking it on.
- **D2 demand-filtered:** a maintainer/bot closes it specifically for insufficient
  upvotes or demonstrated demand. This is useful as a prioritization signal, not
  evidence that OpenAI dislikes the underlying feature kind.
- **X administrative/ambiguous:** duplicate, withdrawn, invalid, spam, question
  answered by existing behavior, no response, author self-closure, unexplained
  `NOT_PLANNED`, or `COMPLETED` without adoption evidence.

Only A1/A2 support the adopted categories. Only D1 supports substantive
"does not like to take on" categories. D2 may support one clearly labeled
"insufficient demand" category or a cross-cutting note. X supports neither.

Maintainer evidence usually comes from an OpenAI-associated account or from a
merged `openai/codex` PR. Do not assume an unfamiliar commenter is a maintainer;
verify from association, account identity, or the linked merged PR.

## Substantial-code versus idea rubric

Classify what the **issue proposer brought before the outcome was decided**:

- **C substantial code:** the proposer supplies a working fork/branch/patch/PR,
  or substantial runnable code plus meaningful validation, implementing most of
  the requested behavior.
- **I idea only:** prose, UX mockup, config example, pseudocode, reproduction,
  proposed API, or "happy to contribute" without a substantive implementation.
- **U unclear:** inaccessible link, unclear authorship/timing, or insufficient
  evidence. Exclude U from the C:I denominator and explain it.

A later implementation by an OpenAI maintainer is still I for this measurement.
A tiny one-line suggestion or isolated snippet is I. Report C:I separately for
A1/A2 and D1 when sample sizes permit, plus the combined qualified ratio.

## Sampling method

Review the assigned, non-overlapping closed-date range. Target 80-120 deeply
screened issues, split about evenly between `reason:completed` and
`reason:"not planned"`. Stratify across months and feature-area labels; do not
take only the newest search page, only issues with linked PRs, or only popular
issues. If a month is sparse, redistribute its quota within the assigned range.

For candidate discovery, authenticated `gh` queries are preferred. Fetch bodies,
comments, state reasons, labels, authors, close dates, and closing PR references.
Inspect the linked PR when it determines adoption or code-provenance status.
Keep API requests batched; GitHub search has a low secondary rate limit.

Useful query shape:

```sh
gh issue list -R openai/codex \
  --state closed \
  --label enhancement \
  --search 'reason:completed closed:YYYY-MM-DD..YYYY-MM-DD' \
  --limit 100 \
  --json number,title,url,stateReason,body,comments,author,labels,createdAt,closedAt,closedByPullRequestsReferences
```

Use `reason:"not planned"` for the comparison pool. Search results are capped,
so query monthly or in other bounded slices when necessary. Do not repeatedly
fire many one-result count queries.

## Seed hypotheses to test, merge, split, rename, or reject

Provisional adopted patterns from the initial sample:

- narrow parity/completeness gaps in an existing interface;
- visibility or machine-readable exposure of state Codex already has;
- bounded ergonomics in established workflows and interaction conventions;
- capability-driven provider/protocol interoperability;
- contained platform compatibility that preserves the main abstraction.

Provisional declined patterns:

- redundant controls where prompts, skills, hooks, OS controls, app-server, or
  an existing command already solve the use case;
- niche vendor/platform integrations whose maintenance cost exceeds reach;
- large architectural exceptions that violate lifecycle or API invariants;
- reversals of deliberate deprecations, defaults, or product direction;
- abstractions that are unreliable or belong to model behavior rather than the
  harness, plus requests that fail the repository's demand/upvote threshold.

These are not required answer headings. Prefer the taxonomy best supported by
your sample, and flag counterexamples.

## Required output

Write only your assigned `issue-survey/ACCEPTED-ENHANCEMENTS-{N}.md`. Include:

1. Scope and exact sampling method, including date range and screened counts.
2. Outcome audit counts for A1, A2, D1, D2, and X.
3. About five adopted categories, each with a short inference and 2-5 linked
   examples carrying their evidence tier.
4. About five declined categories, each with a short inference and 2-5 linked
   examples carrying their evidence tier; keep D2 visibly distinct from D1.
5. C/I/U counts and ratios for adopted, declined, and combined qualified sets.
6. A compact evidence table for every A1/A2 and D1 issue used in categories:
   issue, outcome evidence, category, and C/I/U classification.
7. Counterexamples, ambiguities, sampling bias, and any taxonomy changes you
   recommend for the later synthesis.

Link directly to GitHub issues/PRs. Paraphrase discussion; quote only when a few
words are decisive. Do not infer that code caused acceptance, or that a feature
kind is categorically unwanted, merely from one issue.

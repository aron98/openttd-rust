# Stage-based development

The [roadmap](docs/roadmap.md) defines the product goal, stage order and acceptance
gates. The integration lead coordinates implementation and verification. The
maintainer reviews and merges each stage PR. This workflow replaces the earlier
practice of pushing completed implementation directly to `main`.

## Lifecycle

1. **Analyze the current stage.** Inspect the pinned original source and current
   Rust implementation. Record the exact scope, missing behavior, dependencies,
   acceptance scenarios and test commands. Identify what can run in parallel,
   what must be sequential and which files/interfaces are shared. Put the durable
   stage plan and checklist in tracked `docs/stages/` as part of the stage PR;
   private working notes may remain in ignored `docs/superpowers/`.
2. **Divide ownership.** Assign subagents bounded slices with explicit files or
   modules, dependency contracts and expected evidence. Use isolated worktrees
   when edits may overlap. Agents must preserve others' work. The integration
   lead owns shared decisions, integration and end-to-end checks. If agent limits
   prevent the planned split, report the limitation and sequence the work; do
   not claim unavailable parallel execution or independent review.
3. **Implement and integrate atomic commits.** Branch the stage from the latest
   merged `main`, using a name such as `stage/01-compatibility-contract`. Keep
   each behavioral increment with its direct tests and relevant documentation.
   Verify increments before committing. Integrate worker commits into the stage
   branch, preserving their atomic boundaries. Do not push stage work to `main`.
4. **Verify the integrated stage.** Run relevant unit/integration tests, original
   engine comparisons, negative controls, real CLI/UI/network scenarios and
   existing regression checks. Code changes require formatting, strict Clippy
   and workspace tests as applicable. Exercise external-reference tests through
   their drivers; a skipped test is not a pass. Documentation-only changes need
   content/link/diff checks, not unrelated simulation runs. Record commands,
   results, tested commit and reproducible evidence, including limitations.
5. **Obtain independent review.** Reviewer subagents check requirements, source
   fidelity, code quality and evidence. An author cannot be the sole reviewer of
   their own work. Resolve blocking findings, rerun affected checks and obtain
   review of the fixes. The integration lead verifies the resulting stage and
   accounts for every acceptance criterion. Missing or inconclusive review is
   not approval.
6. **Hand off one stage PR.** Push the stage branch and open a PR against `main`.
   A draft may be opened earlier for visibility, but mark it ready only when the
   acceptance checklist, independent review, relevant local checks and required
   CI checks pass. Include scope, behavior, atomic commit summary, evidence,
   limitations and any compatibility impact. Keep evidence useful to a reviewer
   who does not have the author's local ignored artifact directories.
7. **Wait for the maintainer's merge.** Address feedback on the same branch with
   focused commits and updated checks. Do not merge, enable auto-merge or start
   implementing the next stage. Review approval, green CI or closing a PR without
   merging does not satisfy this gate. A partially implemented stage remains in
   progress; do not relabel it complete to advance the roadmap.
8. **Advance after observing the merge.** Confirm the PR is merged through GitHub,
   fetch the resulting `main`, and use it as the next stage's base. Preserve useful
   evidence and remove merged temporary branches/worktrees. Record the completed
   stage's PR and merge commit in the next roadmap update, then analyze the next
   stage. Never infer a merge from elapsed time or a local approval message.

## Status and scope rules

Use these meanings consistently in the roadmap, stage plan and PR:

- **Planned:** no implementation for the stage has begun.
- **In progress:** analysis, implementation or verification is underway.
- **Ready for review:** the stage meets its acceptance criteria and automated and
  independent review gates; the maintainer's merge is still pending.
- **Completed:** the maintainer has merged the stage PR into `main`.

Parallelism is inside the active stage. Content, networking and browser
portability work may be prerequisites within that stage's written scope, but
must not become a second unreviewed stage. If a stage is too large for a useful
PR, propose revised boundaries and get agreement before splitting it. Do not
change the required compatibility outcome to fit implementation convenience.

The agreed one-PR-per-stage structure groups atomic commits for review. Prefer
a merge method that retains those commits. The maintainer controls the merge;
agents must not rewrite history, amend commits or force-push without explicit
authorization. If a squash merge is chosen, confirm the PR's merge and final
tree before cleanup rather than relying only on commit ancestry.

## PR handoff checklist

- Stage scope, dependencies, ownership and acceptance criteria are documented.
- The branch contains only the stage and its necessary supporting changes.
- All acceptance scenarios have evidence tied to the reviewed commit.
- Applicable tests, native comparisons, negative controls and CI pass.
- Reviewer findings are resolved; no required review is missing.
- Current support and remaining limitations are stated without overclaiming.
- The maintainer can reproduce checks or access retained CI/PR evidence.
- The PR is awaiting the maintainer; the next stage has not started.

This roadmap/workflow documentation is a preliminary documentation change. Its
review does not claim that stage 1's compatibility contract is implemented.

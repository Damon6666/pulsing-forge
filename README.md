# Pulsing Forge

**MCP-native multi-agent infrastructure — the postal system, ledger, and circuit breakers for agent teams.**

> Agents can be wrong. With Forge, they are never lost, never silent, never out of lane.

## What it is

Forge gives every agent a **mailbox** and your team a **ledger**:

- **4 MCP tools** are the entire agent-side contract — `pulsing_send`, `pulsing_broadcast`, `pulsing_drain_mail`, `pulsing_health`. Any MCP agent runtime works: codex, claude-code, cursor, or your own script. Zero runtime modifications, ever.
- **Every letter is journaled exactly-once.** Replay any run, audit any decision, bill any scenario. Your agent team gets a flight recorder.
- **Protocol legality is enforced by the tool layer, not prompts.** Recipient ACL, self-echo circuit breaker, broadcast collapse, auto-silenced review replies — the failure modes we tamed in 50+ stress rounds ship as primitives.

## The 5-minute path (Phase 0 target)

```bash
git clone https://github.com/Damon6666/pulsing-forge && cd pulsing-forge
cargo build --release
forge run scenarios/code-review.toml
```

A 10-agent write-review loop runs on your machine: writer drafts, 8 specialist reviewers each check exactly one thing, a judge arbitrates with a machine-built verdict table, and the run closes via dual-channel closure. Live swimlane dashboard opens in your browser; a full audit report and sequence diagram land in `runs/<timestamp>/`.

## Why multi-agent at all

Not "more agents = better". Four gains you cannot get from one agent with tools:

1. **Parallelism** — fan out, collect by gate.
2. **Specialization** — each reviewer owns one duty; small fresh contexts, no context rot.
3. **Checks and balances** — AI reviewing AI catches real bugs (a planted scikit-learn data-leakage trap: caught by 5 independent reviewers, fixed, re-verified, all on-ledger).
4. **Accountability** — every arrow in the diagram is a letter in the ledger.

## Project layout

| Crate | Role |
|---|---|
| `forge-core` | Gates: collect gate, verdict table, registry+relay, dual-channel closure, budget guard |
| `forge-runtime` | Tide runtime: shops, mailboxes, journal, ACL, per-turn ephemeral agents |
| `forge-cli` | `forge init / run / audit` + built-in dashboard |

Scenarios are **data, not code** — see `scenarios/` and `docs/scenario-schema.md`.

## Documentation worth reading first

- [`docs/failure-taxonomy.md`](docs/failure-taxonomy.md) — 15 failure modes of LLM agent teams, each with the structural fix that tamed it. The hardest-won file in this repo.
- [`docs/scenario-schema.md`](docs/scenario-schema.md) — one TOML describes a team.

## Status

| Phase | Content | State |
|---|---|---|
| 0 | Core gates (collect/verdict/registry-relay/closure), runtime (hub/journal/ACL), CLI `forge run` | ✅ Shipped |
| 1 | 3 scenario library, adapter interface, deterministic CI gate | ✅ Shipped |
| 2 | Consistent-hash sharding (`forge-core::Ring`), GitHub Actions CI, registry spec | ✅ Shipped |
| 3 | Ledger format spec (`docs/ledger-format.md`), control-port auth (`forge-core::TokenRing`) | ✅ Shipped |

**17/17 unit tests green · 3/3 scenarios PASS on CI · zero LLM dependency in the deterministic tier.**

Substrate hardened by a 50-round stress campaign (order-correctness 93%+, zero letter loss). See [`docs/failure-taxonomy.md`](docs/failure-taxonomy.md) — 15 failure modes, each with its structural fix.

## License

Apache-2.0

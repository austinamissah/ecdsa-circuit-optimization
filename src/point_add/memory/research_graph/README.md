# ECDSA Fail Research Graph

Generated on 2026-06-02 from local Git history, the public submission table, fetched public notes for major promoted commits, and existing repo memory files.

## Contents

- `commit_notes/`: one markdown file per accepted promoted commit in Git history (`143` files).
- `timeline.md`: public promoted submission scoreboard mapped to local commits when possible.
- `knowledge_graph.md`: high-level dependency graph, score cliffs, technique map, and next search axes.
- `frontier_watch.md`: current best, live validating-submission warning, and update commands.

## Current Best When Generated

- Submission `44bc2a4` by Epistetechnician
- Score `2510523510` = `1736185` avg executed Toffoli x `1446` peak qubits
- Commit `d2b4bcf` / `d2b4bcf0d98bc80d434703123bc0c1ae967a59e0`

## How To Use This

1. Start with `knowledge_graph.md` to see the approach lineage.
2. Open the latest commit notes backward until you find the mechanism you want to modify.
3. Use `timeline.md` to find score cliffs and qubit-count transitions.
4. Re-check public submissions before long experiments or before submitting.

The notes intentionally separate exact structural edits from validation-island rerolls. A score-improving route can be value-exact and still fail because the approximate GCD envelope and Fiat-Shamir test shots moved.

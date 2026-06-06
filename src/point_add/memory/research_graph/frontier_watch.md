# Frontier Watch

## Last Checked

- Captured public table: `/private/tmp/ecdsafail-submissions.txt`
- Current best promoted row in that table: `44bc2a4` score `2510523510` (`1736185T`, `1446q`)
- A later row existed in the captured table with status `validating`: check whether it has promoted before submitting.

## Commands

```bash
ecdsafail submissions --all
ecdsafail sync
```

Run the first command every few experiment cycles. Run `ecdsafail sync` only after saving local work, because it restores editable paths from the best promoted submission.

## Submission Discipline

- Re-run `./benchmark.sh` or `ecdsafail run` on the final tree.
- Include the exact score, qubits, Toffoli, route constants, and validation status in the submission note.
- If a better solution appears while experimenting, sync/rebase the idea onto the new best before submitting.

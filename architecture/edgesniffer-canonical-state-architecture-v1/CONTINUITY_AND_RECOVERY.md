# Continuity and Recovery Contract

## Hard-wall events

`lagged`, disconnect without exact replay, receive error with ambiguous gap, queue overflow, parser contradiction, duplicate identity conflict, canonical sequence gap, journal corruption, disk failure preventing group commit, producer source-instance change, or checkpoint mismatch immediately seals the current epoch. The receiver writes an immutable failure receipt containing last committed sequence/slot/event hash, detection time, cause, diagnostics hashes and affected interval.

Previously sealed decision packages remain valid. Future observations are paused until a new epoch is created; no later record may bridge the failed interval.

## Recovery

1. Stop acceptance, seal current journal segment and emit failure receipt.
2. If exact provider replay is available, replay into a *recovery verifier*; accept only if identities/sequences prove no missing/contradictory records. Otherwise the interval stays excluded and is written as a blackout interval in the coverage ledger.
3. Create a new source instance/epoch with new boundary receipt and a recovery interval in the coverage ledger.
4. Restore exact checkpoint only if it binds to the proven contiguous replay. Otherwise bootstrap a new candidate universe as specified in `CANDIDATE_UNIVERSE_RECOVERY.md`; pre-gap state is never assumed continuous.
5. Resume decision-local acceptance only when candidate universe, wallet ledger and all required feature states are valid at that decision boundary. Record warm-up/exclusion intervals in the coverage ledger.

Decision-local validity is a future protocol: a later hard wall does not retroactively invalidate an already sealed decision. It never reclassifies V6.

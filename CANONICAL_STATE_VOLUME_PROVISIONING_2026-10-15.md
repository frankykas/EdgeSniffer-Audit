# Canonical State Volume Provisioning - 2026-10-15

## Result

The newly provisioned VirtIO disk was initialized only after a destructive-operation
preflight established that it was the unused `/dev/vdd` device. No existing
EdgeSniffer volume, evidence path, or service was modified.

| Item | Verified result |
| --- | --- |
| Approved device | `/dev/vdd`, new 1.5 TiB VirtIO disk |
| Safety preflight | no partitions, filesystem signatures, mounts, or active references before initialization |
| Partition table | GPT |
| Created partition | `/dev/vdd1`, full-disk ext4 |
| Mount | `/mnt/edgesniffer-canonical-hot` |
| UUID | `80ece7b4-5dc8-40a8-a8d7-6a2607495bc6` |
| Available capacity after format | `1,503,631,032,320` bytes (about 1.4 TiB) |
| Filesystem state | clean |
| Ownership/mode | `root:root`, `0750` |

Persistent mounting is UUID-based in `/etc/fstab`:

```fstab
UUID=80ece7b4-5dc8-40a8-a8d7-6a2607495bc6 /mnt/edgesniffer-canonical-hot ext4 defaults,noatime,nofail,x-systemd.device-timeout=30 0 2
```

`findmnt --verify --tab-file /etc/fstab` completed with no parse or mount errors.
The ext4 label was automatically truncated by the filesystem to its supported
length; the UUID above is the authoritative mount identity.

## Existing evidence and services

The root filesystem, `/mnt/edgesniffer-kefi-hot` (`/dev/vdc1`), the research
volume, and NFS remained mounted on their original devices before and after the
operation. No evidence was moved or deleted.

The V8 service remains `inactive`, and its capacity-pause marker remains present.
It was not restarted and the marker was not removed.

## Canary deployment preparation

The reviewed deployment template now requires the dedicated mount and checks
that it resolves to `/dev/vdd1`. Its new output root is:

```text
/mnt/edgesniffer-canonical-hot/canonical-state-v1-canary-next
```

The corrected storage policy remains frozen in the template:

| Guard | Value |
| --- | ---: |
| Minimum startup free space | 181 GiB |
| Runtime protected reserve | 160 GiB |
| Maximum total canary output | 6 GiB |

The real production dependency graph, with the reviewed storage-gate source
substituted only in an isolated build tree on the new volume, passed all three
canary unit tests using the already-installed compatible Rust toolchain:

```text
3 passed; 0 failed
```

This template and isolated build are not deployed. The installed unit and live
binary were not changed, and no canary was started.

## Remaining gate before a live canary

The old capture-only producer does not yet integrate the required finalized-RPC
dispatcher. A live canary remains blocked until that real integration, its
restart-recovery coverage, and its offline integration tests are complete and
reviewed. A matched finalized slot must still not be described as a complete
scientific market interval or candidate universe.

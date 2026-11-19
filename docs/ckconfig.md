# The CKCONFIG partition (#22, #23)

Per-stick data (identity, token, settings, wallpaper) moves from the EFI partition to a small
partition of its own, so the generic image stays identical for every stick and Forge changes only the
start of the disk. Constants are in `ck-protocol` (`CONFIG_PARTITION_LABEL`, `CONFIG_MOUNT_POINT`,
`CONFIG_FILE`, `ESP_CONFIG_*`). The image side is lane A (task A5); the reader, the Forge writer and
the thin-image rework are lane B (task B4).

## Layout

The generic image already has this order (`ck-os/tools/build-image`); nothing about it changes:

| # | Size | Type | Content |
|---|---|---|---|
| 1 | 1 MiB | BIOS boot | GRUB for legacy boot |
| 2 | 128 MiB | ESP, FAT32, label `EFI` | boot loader only, no settings |
| 3 | 16 MiB | basic data, FAT16, label `CKCONFIG` | the stick's settings |
| 4 | rest | Linux, ext4, label `CKROOT` | the system |

- `CKCONFIG` is mounted **read-only** at `/ck-config` by a unit before `ck-config.service`, with
  `uid=0,gid=0,umask=0077` so only root reads the token (a FAT file system has no owners; this is
  what the ESP mount does today).
- `config.json` is at the root of the partition: the `StickConfig` document, schema 2.
- The wallpaper is a file beside it, named by `settings.branding.wallpaper` (a plain file name, no
  folders), at most 4 MiB.
- Empty on the generic image: a fresh image has the file system and no `config.json`. The agent then
  keeps the image defaults, as it does today without settings.
- Request to lane A: also set the GPT partition *name* to `CKCONFIG` (`name=CKCONFIG` in the
  `sfdisk` input), so Forge finds the partition without guessing from its position.

## Thin images

A thin image is the start of the disk plus a manifest naming the base (`ck_writer::thin`). Today the
head ends at the end of the ESP. **Decision: keep the layout and extend the head** to the end of the
`CKCONFIG` partition (about 146 MiB with the sizes above, instead of 129 MiB). Moving `CKCONFIG`
inside the first 130 MiB would mean shrinking the ESP, which holds the boot loaders.

- Forge reads the partition table and takes the end of the last of ESP and `CKCONFIG` (found by GPT
  name, then by label). The head length is no longer a constant, and the manifest already stores
  `head_len`, so thin images made earlier (ESP only) still open and write as before.
- Partition 3 must follow the ESP with nothing between them, so the head is one contiguous range. The
  build checks this; Forge refuses a base where it is not.
- An old ICPC-based image has no `CKCONFIG`: Forge keeps writing the ESP copy for it.

## Reader (agent)

`ck-config` looks for settings in this order, and uses the first it finds:

1. `/ck-config/config.json` (schema 1 or 2).
2. `/boot/efi/ck/config.json` (the old place, schema 1 or 2), kept for **one release** so sticks made
   before this change still boot.

The stored copy under `/etc/ck/` and its hash work as before. After the release with the fallback,
step 2 and the ESP writer are removed.

## Forge writer

After the image is written, Forge formats nothing: the file system exists on the image. It writes
`config.json` and the wallpaper into partition 3 with `ck-image` (a FAT writer that takes a
partition, now used for the ESP only). The token is only in that file, never in the manifest, the
history or a log. Forge verifies by reading the file back.

## Later (#24)

A one-time enrolment code instead of a long-lived token changes the content of `config.json`, not
the layout.

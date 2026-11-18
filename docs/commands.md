# Hub commands to a stick (#1)

The Hub can ask a stick to show a message, lock the screen or shut down. This note fixes the data
types in `ck-protocol` (`command.rs`) and the signing helpers in `ck-crypto`. Delivery and execution
are in the Hub (lane C) and the agent (`ck-os`).

## Delivery

Sticks sit behind the venue network and the Hub cannot call them, so commands travel in the reply to
the stick's heartbeat: `HeartbeatReply { commands: [SignedCommand] }`. A reply with no commands is
`{}` or `{"commands":[]}`; an older stick ignores the field. Commands are pulled at the heartbeat
interval (15 s by default), so a command reaches the stick within that time.

The stick answers with an event of kind `command_ack` (payload: `CommandAck`), sent through the
normal event queue, so the Hub sees the outcome and stops re-sending that command.

## The command

```
Command { id, stick_id, issued_at, expires_at, action }
action = message { text } | lock_screen | shutdown
SignedCommand { command, signature }
```

- `id`: 1 to 64 letters, digits or `-`, unique per command (the Hub uses a UUID).
- `stick_id`: the one stick it is for. A command for another stick is refused.
- `issued_at`, `expires_at`: UTC, RFC 3339. The window is at most 1 hour (`MAX_LIFETIME_S`).
- `message.text`: 1 to 500 characters, shown as a desktop notification.
- `signature`: HMAC-SHA256 of `Command::canonical_bytes()` (the compact JSON of `command`) with the
  stick's token, the same secret that signs events. Hex, lowercase.

## What the agent checks, in order

1. The signature is valid (`ck_crypto::verify_command`).
2. `Command::check(stick_id, now)`: the command is for this stick, not expired (`now < expires_at`),
   not from the future (`issued_at <= now + 5 minutes`, for clock skew), valid for at most an hour,
   and the id and text are well formed.
3. Replay: the `id` is not in the agent's list of executed ids. The agent keeps ids until their
   `expires_at` has passed (an expired command is refused anyway), so the list stays small. It is
   kept on disk so a reboot does not forget it.

Any failure sends an ack with result `refused` and a reason (`bad_signature`, `wrong_stick`,
`expired`, `not_yet_valid`, `bad_lifetime`, `replayed`, `malformed`) and runs nothing. Running and
failing sends `failed`; success sends `ok`. The ack never contains the message text.

## Not in scope

Per-contest keys (H.4): commands are signed with the stick token for now. When H.4 lands, only the
key given to `sign_command` and `verify_command` changes.

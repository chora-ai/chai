# Journey: Matrix (Experimental) — receive message and reply

**Goal:** Confirm the gateway is logged into a Matrix homeserver, messages in a **joined** room are received (including **encrypted** rooms when keys are available), and the agent's reply is sent back as `m.room.message` (`m.text`).

**Background:** [Connections → Matrix](../guides/04-connections.md#matrix) · [Configuration → Channels](../guides/03-configuration.md#configuring-channels)

The gateway uses **[matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk)** with a **SQLite** store for state and **E2EE** keys at **`<active-profile>/matrix`** under **`~/.chai/profiles/<name>/`**. It acts as **one Matrix user** (the account you configure). Unlike Telegram's dedicated bot API, Matrix uses a normal user account reserved for Chai; invite it into rooms, and message from another client (e.g. Element).

**Encryption:** Encrypted rooms are supported: the SDK decrypts inbound timeline events and encrypts outbound sends when the room is encrypted. **Interactive device verification (SAS)** can be completed **without Element** using gateway HTTP under **`/matrix/verification/*`** (same host and port as the WebSocket gateway). Element remains an option if you prefer to verify there. Details: [base/ref/MATRIX.md](../../base/ref/MATRIX.md).

## Prerequisites

- **Setup complete** — You have installed chai with the `matrix` feature (`cargo install --path crates/cli --features matrix` or `cargo run -p cli --features matrix -- gateway`), run `chai init`, and verified Ollama is available (see [00-setup-init.md](00-setup-init.md)).
- A **Matrix account** for the gateway (dedicated is best): you will use its **access token** or **password** below.
- **Homeserver** URL, e.g. `https://matrix.example.org` (your server or a public one that allows your account).
- **Config** — one of:
  - **`channels.matrix.homeserver`** + **`channels.matrix.accessToken`**, and **`channels.matrix.userId`** if needed, plus **`channels.matrix.deviceId`** if **`GET /account/whoami`** does not return a device id for your token, **or**
  - **`channels.matrix.homeserver`** + **`channels.matrix.user`** + **`channels.matrix.password`** for password login at startup (the SDK creates a session and persists keys under the store path).
- **Environment overrides** (optional): `MATRIX_HOMESERVER`, `MATRIX_ACCESS_TOKEN`, `MATRIX_USER_ID`, `MATRIX_USER`, `MATRIX_PASSWORD`, `MATRIX_DEVICE_ID` — see [Configuration → Environment Variables](../guides/03-configuration.md#environment-variables).
- **Ollama** (or your configured default provider) running with the default model. If the model is missing or the provider is down, the message may be received but no reply is sent (see verification below).
- A **room** where the Chai Matrix user is **joined** (invite `@your-chai-user:server` from Element or another client). The room may be **encrypted** or not.
- **Gateway URL** for optional HTTP checks — note **`gateway.bind`** and **`gateway.port`** in config (default port **15151**). Examples below use `http://127.0.0.1:15151`; substitute your bind/port.

## Steps

1. **Configure** **`~/.chai/profiles/<active>/config.json`** with **`channels.matrix`** as above, **or** export the **`MATRIX_*`** variables for a one-off test.

2. **Start the gateway**
   - From repo root: `cargo run -p cli --features matrix -- gateway`
   - Or: `chai gateway` (if installed with `--features matrix`)
   - Optional: `RUST_LOG=info` for channel logs.
   - **Expect:** Log lines like `matrix: session restored` or `matrix: logged in with password` and `matrix channel: starting sync loop`.
   - The first **sync** completes without turning historical timeline into agent messages; messages **after** the gateway is running are what matter.

3. **Open the room** in Element (or another Matrix client) as **a different user** (or the same account in another session — but typically you chat **into** the room from your personal account while Chai runs as the invited user).

4. **Send a text message** in that room (plain text; the gateway only handles `m.text`).

5. **Verify receipt and reply**
   - **In the room:** The Chai user should post a reply with the model's response.
   - **In logs (optional):** With `RUST_LOG=info` or `RUST_LOG=debug`, look for agent activity; failures may show `inbound: agent turn failed` or send errors.

6. **Stop the gateway** with Ctrl+C in the gateway terminal.

## Room Allowlist (optional)

To restrict which rooms produce agent turns (recommended on public homeservers):

- Set **`channels.matrix.roomIds`** to a JSON array of room ids (e.g. `["!abc:example.org"]`), **or**
- Set **`MATRIX_ROOM_ALLOWLIST`** to a comma-separated list (overrides config when set and non-empty).

If unset or empty, **all joined rooms** receive inbound messages as before. If set, messages from other rooms are ignored for the agent (no reply).

Repeat the main **Steps** above; messages must come from an **allowlisted** room id.

## Interactive Verification (SAS) via Gateway (optional)

Use this when another client or device wants to verify the gateway's session and you do **not** want to use Element on the gateway side.

**Auth:** If the gateway uses a connect token (**`CHAI_GATEWAY_TOKEN`** / **`gateway.auth.token`**), send **`Authorization: Bearer <token>`** on every request below. If there is **no** token, these routes work only when **`gateway.bind`** is loopback (**`127.0.0.1`**, **`::1`**, or **`localhost`**).

1. From the **other** client (e.g. Element), start **device verification** / **verify** against the Chai user's device (your homeserver's UI).

2. **List pending requests** (gateway must be running with Matrix connected):

   ```bash
   curl -s http://127.0.0.1:15151/matrix/verification/pending
   ```

   With a gateway token:

   ```bash
   curl -s -H "Authorization: Bearer YOUR_GATEWAY_TOKEN" http://127.0.0.1:15151/matrix/verification/pending
   ```

3. **Expect:** JSON with a **`pending`** array; each entry uses **camelCase** **`userId`**, **`flowId`**, **`fromDevice`** (same field names as POST bodies).

4. **Accept** the request (reuse **`userId`** and **`flowId`** from **`pending`**):

   ```bash
   curl -s -X POST http://127.0.0.1:15151/matrix/verification/accept \
     -H 'Content-Type: application/json' \
     -d '{"userId":"@them:example.org","flowId":"..."}'
   ```

   Add **`-H "Authorization: Bearer …"`** when the gateway requires a token.

5. **Start SAS** and **fetch emoji or decimals** (repeat **`userId`** / **`flowId`**):

   ```bash
   curl -s -X POST http://127.0.0.1:15151/matrix/verification/start-sas \
     -H 'Content-Type: application/json' \
     -d '{"userId":"@them:example.org","flowId":"..."}'

   curl -s "http://127.0.0.1:15151/matrix/verification/sas?userId=@them%3Aexample.org&flowId=..."
   ```

6. **Compare** the short auth string with the other client; if they match:

   ```bash
   curl -s -X POST http://127.0.0.1:15151/matrix/verification/confirm \
     -H 'Content-Type: application/json' \
     -d '{"userId":"@them:example.org","flowId":"..."}'
   ```

   If they do not match, use **`/matrix/verification/mismatch`** instead. To abort, **`/matrix/verification/cancel`**.

Full route list and behavior: [base/ref/MATRIX.md](../../base/ref/MATRIX.md).

## How to Get an Access Token (Element Web / Desktop)

Use **Settings → Help & About → Access Token** (wording varies) or your homeserver's documented flow. Prefer a **dedicated** account for automation. If **`whoami`** returns **`device_id`**, you may not need **`MATRIX_DEVICE_ID`**.

## `/new` and Session

Send **`/new`** (case-insensitive) in the room to start a **new session** for that room (same as other channels).

## If Something Fails

- **`matrix: restore_session failed`** — Token invalid, wrong **`device_id`**, or store out of sync with the server; try password login once or clear **`<active-profile>/matrix`** (you will need to verify devices again).
- **`matrix: room not loaded yet`** — Wait for sync after joining; send again after the room appears in the client state.
- **No reply** — Provider/model down; check `inbound: agent turn failed`.
- **Allowlist** — If you configured **`roomIds`** / **`MATRIX_ROOM_ALLOWLIST`**, confirm the room id matches exactly (including `!` and server part). Logs may show `ignoring message from non-allowlisted room`.
- **Verification HTTP** — **404** on **`/matrix/verification/*`** means Matrix did not connect (check Matrix config/logs). **401** / **403** means auth or bind: use **`Bearer`** token or bind the gateway to loopback for tokenless mode. **404** on accept/start/sas — wrong **`userId`** or **`flowId`**, or the crypto machine has not yet recorded the request (retry after a sync).

## Wire-only Check (no Gateway)

`cargo run -p chai-spike --bin matrix-probe` — see **`crates/spike/README.md`**.

## Summary

| Step | Action |
|------|--------|
| Store | **`<active-profile>/matrix`** |
| Config | `channels.matrix` + token or password |
| Allowlist (optional) | `channels.matrix.roomIds` or `MATRIX_ROOM_ALLOWLIST` |
| Verification (optional) | `GET/POST` `http://<bind>:<port>/matrix/verification/*` (see [MATRIX.md](../../base/ref/MATRIX.md)) |
| Gateway | `chai gateway` |
| Room | Invite Chai user; send text |
| Verify | Reply in room; optional logs |

**See also:** [04 — Channel: Telegram](04-channel-telegram.md) · [09 — Channel: Signal](09-channel-signal.md)

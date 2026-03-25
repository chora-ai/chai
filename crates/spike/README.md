# chai-spike

Small **spike** binaries for Matrix and Signal integration research. They are **not** part of the Chai gateway; they validate wire formats and ops assumptions documented in **`.agents/ref/`** and **`.agents/spec/CHANNELS.md`**.

## Matrix (`matrix-probe`)

One password login + one **`/sync`**; prints **`m.room.message`** lines as `room_id<TAB>body` (candidate **`conversation_id`** values).

```bash
export MATRIX_HOMESERVER=https://matrix.example.org
export MATRIX_USER=alice          # or @alice:example.org
export MATRIX_PASSWORD=...
cargo run -p chai-spike --bin matrix-probe
```

## Signal (`signal-probe`)

Expects a running **signal-cli** HTTP daemon:

```bash
signal-cli -a +1234567890 daemon --http 127.0.0.1:7583
```

Then:

```bash
export SIGNAL_CLI_HTTP=http://127.0.0.1:7583   # optional; default shown
cargo run -p chai-spike --bin signal-probe
```

Calls **`GET /api/v1/check`**, **`POST /api/v1/rpc`** with **`listGroups`**, and samples **`GET /api/v1/events`** (SSE). See **signal-cli-jsonrpc(5)** and the [JSON-RPC service](https://github.com/AsamK/signal-cli/wiki/JSON-RPC-service) wiki for **`receive`** notifications and **`send`** parameters.

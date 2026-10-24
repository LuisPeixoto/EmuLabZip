# Mocks and breakpoints

Rules let you change what an app gets back without touching the server: answer a request yourself, make it slow, make
it fail, or stop it and edit it by hand. They live in the proxy, so they work on every device the [Network](network.md)
tool is connected to.

## Rules

A rule has a **matcher** (which requests), and one **action** (what to do). The first enabled rule that matches wins.

| Matcher field | Matches |
|---------------|---------|
| `methods` | Any of these methods. Empty means any. |
| `host` | A pattern: `api.example.com`, `*.example.com`. `*` is any run of characters, `?` any one. |
| `path` | A pattern for the path without the query: `/v1/users/*`. |
| `urlContains` | Text the whole address (host, path and query) has to contain. |

Every field that is set has to match.

| Action | What happens |
|--------|--------------|
| **Answer** (`mock`) | The proxy answers with your status, headers and body, after an optional delay. The server is never asked. |
| **Delay** | The request waits, then goes on. Use it to try a slow network. |
| **Block** | The app gets a gateway error, as if the network had failed. Nothing reaches the server. |
| **Hold** (`breakpoint`) | The request, the response, or both, stop and wait for you. |

Flows that a rule touched show a **MOCK** or **RULE** badge in the list and in the details.

## In the app

Open **Network**, then **Rules**. Pick a preset (a JSON answer, a 500, a slow network, a block, a breakpoint) or start
from a request: select it and press **Mock** to get a rule that answers the way the server did, ready to edit. Rules
apply to the next request and are kept in `net-rules.json` in EmuLabZip's data folder.

### Breakpoints

A held request or response appears in a drawer at the bottom of the Network view. You can edit the headers (one
`name: value` per line), the body (when it is text) and, for a response, the status. **Continue** sends it on with your
edits, **Abort** fails it, **Continue all** lets everything through. A response is shown decoded and goes on
uncompressed. If nobody answers, a held message continues unchanged after 2 minutes, so an app never hangs for good.

## On the command line

```bash
emulabzip net --rules rules.json
```

```json
[
  {
    "name": "stub the profile",
    "matcher": { "host": "api.example.com", "path": "/v1/profile" },
    "action": { "type": "mock", "status": 200,
                "headers": [["content-type", "application/json"]],
                "body": "{\"name\": \"Ada\"}" }
  },
  { "name": "slow uploads", "matcher": { "methods": ["POST"] }, "action": { "type": "delay", "ms": 3000 } },
  { "name": "no analytics", "matcher": { "host": "*.analytics.example.com" }, "action": { "type": "block" } }
]
```

A mock can carry a binary body: set `"base64": true` and give the body as base64. Breakpoints need someone to continue
them, so the command line skips such rules and says so.

## Limits

- A mock body can be up to 5 MB, and a delay up to 60 seconds.
- Rules see the decrypted request, so HTTPS needs the [certificate](network.md#the-certificate) installed. Hosts that
  are passed through (pinned apps) cannot be changed.
- WebSocket upgrades are matched like any request, but only the handshake can be mocked or blocked.

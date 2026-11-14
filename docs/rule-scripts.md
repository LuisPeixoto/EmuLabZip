# Rule scripts

A **script** is a small WebAssembly program that looks at a request or a response and decides what happens to it: add a
header, hide a field, answer on its own, fail one request in three. A rule with the action **Script** runs it for every
request the rule matches.

A script is sandboxed. It cannot read files, reach the network, read the clock or touch anything else on your computer:
the only function it can import is `env.log`. It is stopped after a fixed amount of work and a fixed amount of memory. And it
runs only after **you approved its exact bytes**.

## Approving

A script file does nothing until it is approved, in the rule's dialog (**Approve this script**, which shows its SHA-256 and
what it looks at) or on the command line:

```bash
emulabzip script show add-header.wasm       # what it does, and whether it is approved
emulabzip script approve add-header.wasm
emulabzip script list
emulabzip script revoke 09e74ff6            # a file, or the start of its hash
```

The approval is for the file's SHA-256: change one byte and it has to be approved again. Approve only scripts you trust: a
script sees the requests and responses its rule matches, with their headers and bodies. A flow whose script did not run says
why in its details.

## The interface

A script is a module that exports:

| Export | Type | |
|--------|------|-|
| `memory` | memory | The script's linear memory. |
| `alloc` | `(len: i32) -> i32` | A pointer to `len` bytes the host may write into. |
| `on_request` | `(ptr: i32, len: i32) -> i64` | Called with a request. Optional. |
| `on_response` | `(ptr: i32, len: i32) -> i64` | Called with a response. Optional. |

At least one of `on_request` and `on_response` is needed. The host writes a JSON document at `ptr` (`len` bytes) and the
function returns `(out_ptr << 32) | out_len`, where its answer, also JSON, is.

**Input**

```json
{ "method": "GET", "url": "https://api.example.com/users?id=1",
  "headers": [["accept", "application/json"]],
  "body": "<base64>", "bodyOmitted": false,
  "status": 200 }
```

`status` is only there for a response. A response body is decoded (gzip, brotli, zstd). A body of more than 1 MB is not
handed over: `bodyOmitted` is `true` and `body` is empty.

**Answer**

```json
{ "action": "continue", "status": 201, "headers": [["x-by", "me"]], "body": "<base64>", "text": "…", "message": "…" }
```

- `"continue"` (or no `action`) lets the request go on or the response through. `headers`, `body` (base64) or `text`, and for a
  response `status`, replace what was there; what is left out is not changed. A header list **replaces** the whole list, so
  start from the input's.
- `"replace"` (for a request) answers at once with that `status` (200 by default), `headers` and body, and the server is
  never asked. For a response it is the same as `continue`.
- `"block"` fails the request with a gateway error, saying `message`.

A script that traps, runs out of work, answers something that is not valid, or asks for something it is not given does **not**
break the request: it goes through as if the script were not there, and the reason is in the flow's details.

## Limits

- **Work:** 10 million units of fuel for each call, roughly the same number of instructions.
- **Memory:** 64 MB. A module can be 2 MB, and an answer 8 MB. At most 20 lines of `env.log`, 1 KB each.
- A new instance for every call, so nothing a script keeps survives to the next request; use the URL when a choice has to be
  repeatable.
- No imports other than `env.log(ptr: i32, len: i32)`, which writes a line that shows in the flow.

## Writing one

Any language that compiles to WebAssembly works. `tools/rule-scripts` has three examples in Rust, each built for
`wasm32-unknown-unknown`, with a small helper crate that does the packing:

| Script | What it does |
|--------|--------------|
| `add-header.wasm` | Adds `x-emulabzip-script` to every request and response. |
| `redact-json.wasm` | Replaces the value of `email`, `token`, `password` and `authorization` in JSON responses with `***`. |
| `flaky.wasm` | Answers 503 for about a third of the addresses, chosen from the URL so the same one always fails. |

```bash
cd tools/rule-scripts && ./build.sh          # needs: rustup target add wasm32-unknown-unknown
```

The files in `dist/` are built already. Add the script to a rule, approve it, and watch the flow.

# Network rules: files, other servers and contracts

[Mocks and breakpoints](mocks.md) answer, delay, block or hold a request. Four more actions work on the way a request is
answered, and rules can be chained.

| Action | What happens |
|--------|--------------|
| **File** (`mapLocal`) | Answers with a file on this computer, or with the file of a folder that the path names. The server is not asked. |
| **Server** (`mapRemote`) | Sends the request to another server, as if the app had asked for it. |
| **Contract** (`validate`) | Checks the response against an OpenAPI file and shows what does not match. It changes nothing. |
| **Script** (`script`) | Runs a WebAssembly [script](rule-scripts.md) on the request and the response. |

In the app these are in **Network → Rules**, with file pickers. On the command line they go in the same `--rules` file.

## Map local

```json
{ "name": "bundled assets", "matcher": { "host": "cdn.example.com", "path": "/assets/*" },
  "action": { "type": "mapLocal", "path": "/Users/me/app/dist" } }
```

With a folder, the part of the path before the rule's own pattern is left off: with `/assets/*`, a request for
`/assets/app.js` gets `app.js` from the folder, and a request for a folder gets its `index.html`. With a file, the file is the
answer to every request the rule matches. The content type comes from the extension (or from `contentType`), and `status`
(200 by default) is the status of the answer. **Nothing outside the folder is ever served**: `..`, an encoded `%2e%2e`, a
drive or a symbolic link that leads out are refused. A file can be up to 50 MB.

The flow shows **MOCK** and the file that was served.

## Map remote

```json
{ "name": "use my laptop", "matcher": { "host": "api.example.com", "path": "/api/*" },
  "action": { "type": "mapRemote", "scheme": "http", "host": "localhost", "port": 3000, "pathPrefix": "/v2/" } }
```

What the rule leaves out stays as the app asked, so a host and a port alone send the same path to a local server. `pathPrefix`
replaces the part of the path before the pattern's first `*`: `/api/users` becomes `/v2/users`. A new scheme or host takes the
scheme's usual port unless one is given. The new server is told its own host, unless `"preserveHost": true`. The Network view
keeps showing what the app asked and says where the request was sent. Map remote does not apply to gRPC calls.

## Contract

```json
{ "name": "the API contract", "matcher": { "host": "api.example.com" },
  "action": { "type": "validate", "spec": "/Users/me/app/openapi.yaml" }, "thenContinue": true }
```

Each response is checked, after it has been sent, against an **OpenAPI 3.0 or 3.1** description in JSON or YAML (the file is
read when the rule is saved, so a file that cannot be used is refused at once). The operation comes from the method and the
path (`/users/{id}`, with the servers' base paths), the schema from the status (`200`, then `2XX`, then `default`) and the
content type, and the body is validated. The flow gets a **Contract** tab and a badge in the list:

- the response **matches** (`ok`);
- it **does not match**: each problem comes with a JSON pointer into the body (`/items/0/name`) and a message, and a status
  or content type the spec does not describe is one too;
- the spec **does not describe** the request (`unknown`), or has **no schema** to check this response against, or the body is not
  JSON (`unchecked`).

Offline, `emulabzip openapi check --spec api.yaml --har run.har` checks the responses of a HAR file and exits with an error
when one does not match, which is what a CI step wants. Only responses are checked (not requests), only JSON bodies, and
only `$ref`s inside the file. A Swagger 2.0 file is refused with a message to convert it.

## Chaining rules

The first rule that matches decides, unless it says `"thenContinue": true`. A rule that **changes** the request (a delay, a map
to another server, a contract check, a script) can pass the request on to the rules below it; a rule that **answers**, blocks or
holds the request (an answer, a file, a block, a breakpoint) always ends the search. So a delay followed by a file answers
slowly from the file, and a contract check followed by a mock checks the mock. The flow lists every rule that applied.

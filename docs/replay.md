# Replaying a recording

A **replay** answers the app's requests from a recording, so the servers are never asked. The app runs without the
internet, against what a server said once. It is for bugs whose backend has changed since, and for runs in CI that must
give the same result every time.

```bash
# Record once, with the network.
emulabzip session record -o run.ezsession --app com.example.app --network

# Run the app against it. Press Ctrl+C to stop.
emulabzip net --replay run.ezsession
```

A [session](sessions.md) with its network, or any HAR file (a browser's export, or `emulabzip net --har`), is a recording.

## What it does

The proxy checks the recording before it asks a server. A request that is in it gets the recorded answer: the status,
the headers and the body, as they were. A request that is **not** in it fails with a `504` that says *not in recording*,
and shows in the request list marked as failed. It is never sent to the server. That is the point: a test that depends
on an address nobody told you about fails, instead of quietly using the network.

- Requests are matched on the method, the host, the path and the query. The order of the query parameters does not
  matter; the port does not either.
- A request recorded **more than once** gets the answers in the order they were recorded, and the last one again after
  that. A counter that said 1, then 2, says 1, then 2, then 2.
- When requests to the same address differ in their body (a GraphQL query, say), the one with the same body is preferred.
- HTTPS works, because the proxy ends the TLS itself (the device has to trust the [certificate](network.md#the-certificate)
  as for any capture).
- A rule from [Mocks](mocks.md) still wins over the recording, because you wrote it on purpose.

## What it does not do

While a recording is replayed, what cannot be answered from a HAR file is refused instead of being let through to the
internet:

- **WebSockets**. Their messages are in the file, but not replayed.
- **gRPC calls**. A HAR file has no trailers, so the call could not finish.
- **Connections the proxy cannot decode**: other protocols, and apps that do not accept the EmuLabZip certificate.
- **Sending a request again** from the Network view.

Requests that failed when they were recorded are not in the recording.

## Letting some through

```bash
emulabzip net --replay run.ezsession --passthrough
```

With `--passthrough`, a request that is not in the recording goes to the real server. Use it when you only want to freeze
part of what the app does. `--timing` makes each answer wait as long as the server did, for an app that behaves
differently when the network is slow.

## In CI

```bash
emulabzip ci -o run.ezsession --network --replay baseline.ezsession --fail-on-miss -- ./gradlew connectedCheck
```

The run records itself as usual, with its requests answered from `baseline.ezsession`. `--fail-on-miss` fails it when the
app asked for something that is not in the recording, and the end of the run lists what. `session record` takes
`--replay` too. Both need `--network`.

## In the app

In **Network**, **Replay → Choose a file…** loads a HAR file or a session. A bar shows what is being replayed, how many
requests were answered and how many were not (and which, under **Not recorded**). In **Sessions**, **Replay network**
loads the open session. Start the proxy and connect the device as usual; **Stop replay** goes back to the servers.

## Good to know

- The recording is read before anything else, so a wrong file or a session without a network is explained at once.
- The answers are the recorded bodies, decoded. `Content-Encoding`, `Content-Length` and `Alt-Svc` are not repeated, so
  the app is not told to use a compression the body no longer has, nor to try HTTP/3 around the proxy.
- A recording holds what the server sent, including tokens and personal data. Look before you share it.

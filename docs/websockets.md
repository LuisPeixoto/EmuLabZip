# WebSocket messages

The [Network](network.md) tool reads WebSocket traffic as messages, not bytes. Open a WebSocket request and the
**Messages** tab lists what went each way, with the time since the connection opened.

- **Arrows** show the direction: up goes from the app to the server, down comes back.
- Text messages show as text, and JSON is pretty-printed when you open one. Binary messages show a hex dump.
- The filter looks inside text messages.
- Control frames (ping, pong, close) are listed too.

## How it is read

The proxy still copies every byte between the app and the server unchanged and only listens in. It follows RFC 6455:
frames with 7, 16 and 64 bit lengths, masks, fragmented messages joined back into one, control frames in between
them, and `permessage-deflate` when the two sides agreed on it (including `no_context_takeover`). If what flows is not
WebSocket after all, the reading stops quietly and the connection carries on.

## Limits

A flow keeps up to 1,000 messages and 64 KB of each; the tab says how many later messages were not kept, and when a
message was cut. The counts keep going. The [HAR export](network.md#export) includes the messages in
`_webSocketMessages`, as Chrome does.

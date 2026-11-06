# gRPC and protobuf

gRPC calls are shown in **Network** by the method they call, with the status the server answered.

- The **Operation** column shows `Service/Method`, taken from the path.
- gRPC says how a call ended in **trailers**, headers sent after the body (`grpc-status`, `grpc-message`). They are
  recorded and shown in the **Headers** tab, and the status shows in the list as `OK`, `UNAVAILABLE`, `NOT_FOUND`…
- Calls are streamed through the proxy as they happen, so streaming calls work, and a long call shows its messages as
  they come. A request breakpoint cannot hold a gRPC call, because it needs the whole message. Rules can still delay,
  block or answer it.
- Only gRPC over TLS (HTTP/2 negotiated by ALPN) is decrypted, like all HTTPS. Cleartext HTTP/2 (`h2c`) is not
  supported.

## Reading the messages

A gRPC body is a series of messages, each protobuf. The **Request** and **Response** tabs list them as JSON, in one of
two ways:

- **Without a schema.** Protobuf on the wire has field numbers, not names. EmuLabZip reads them anyway: `{"1": "hello",
  "2": 150, "3": {"1": 5}}`. Text is shown as text, a nested message as an object, a repeated field as a list, and
  other bytes as hex. A message that is not protobuf says so.
- **With your `.proto` files.** In **Network → Network → gRPC and protobuf**, choose the folder that holds them (the one
  `import` paths are relative to). They are compiled in memory, with no `protoc` needed. The call's method then says
  which message types its request and response are, and the messages are shown as protobuf's JSON, with the real field
  names. A message that does not fit its type falls back to the reading without a schema.

Messages compressed with `grpc-encoding: gzip` are unpacked.

# GraphQL

In **Network**, a GraphQL request is named after what it asks for, so a list of `POST /graphql` is readable.

- The **Operation** shows before the path: the `operationName` the client sent, or else the name written after
  `query`, `mutation` or `subscription`. An operation without a name is shown as `anonymous query` (or `mutation`).
  A batch (a list of requests in one body) shows the first names and how many more.
- GraphQL requests are found by their body: a JSON `POST` with a `query`, an `application/graphql` body, or a `GET`
  with `?query=`.
- The filter searches the operation names too.
- **Request** tab: the query as it was written, then its variables as JSON, and a **GraphQL** badge. In a batch, each
  operation has its own heading.
- On the command line, `emulabzip net` prints the operation after each request.

The response of a GraphQL call is JSON like any other. A GraphQL server usually answers `200` even when the operation
failed, with the failure in an `errors` list: open the **Response** tab to see it.

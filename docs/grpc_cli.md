<!-- SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions. -->
# gRPC CLI Quickstart

This guide walks through using the [Evans](https://github.com/ktr0731/evans) CLI to interact with the SynaGraph gRPC API.

## 1. Install Evans

Download the binary for your platform and place it on your `PATH`:

```bash
# Linux example
tar -xzf evans_linux_amd64.tar.gz
sudo mv evans /usr/local/bin/
evans --version
```

Homebrew users can alternatively run:

```bash
brew tap ktr0731/evans
brew install evans
```

## 2. Start SynaGraph

From the project root, run the servers:

```bash
cargo run
```

The gRPC listener defaults to `0.0.0.0:50051`.

## 3. Launch Evans in REPL Mode

In a new shell:

```bash
evans --proto proto/synagraph.proto --host localhost --port 50051 repl
```

Evans loads the descriptors and opens an interactive prompt. Select the target package and service before issuing RPC calls:

```text
show package
package synagraph.v1
service GraphService
```

## 4. Call Ping

Inside the prompt:

```text
call Ping
```

When prompted, provide the request message:

```json
{ "message": "hello" }
```

You should receive a response similar to:

```json
{
  "message": "pong: hello",
  "version": "0.1.0"
}
```

## 5. Call UpsertNode

```text
call UpsertNode
```

Sample payload:

```json
{
  "nodeId": "",
  "kind": "note",
  "payloadJson": "{\"title\":\"example\",\"body\":\"Hello Knowlemesh\"}"
}
```

The response echoes the generated node ID and `created` flag:

```json
{
  "nodeId": "1f4f3f12-3b7f-4eb4-9f55-e1fef91b9b2f",
  "created": true
}
```

## 6. Exit

Type `ctrl+d` or `exit` to leave the Evans REPL.

---

For scripted calls, Evans supports non-interactive mode:

```bash
evans --proto proto/synagraph.proto --host localhost --port 50051 cli \
  call --package synagraph.v1 --service GraphService Ping \
  --payload '{"message":"batch"}'
```

To automate the `UpsertNode` smoke test, adjust the RPC name and payload accordingly:

```bash
evans --proto proto/synagraph.proto --host localhost --port 50051 cli \
  call --package synagraph.v1 --service GraphService UpsertNode \
  --payload '{"nodeId":"","kind":"note","payloadJson":"{\"title\":\"example\"}"}'
```

Refer to the official Evans documentation for advanced usage.

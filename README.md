# merged-order-book
Example grpc server project that merges order book bids/asks from binance & bitstamp etcbtc exchanges into a top 10.

```
                 +------------------------+
                 |                        | <--websocket-- binance
<--grpc-stream-- | merged-order-book-grpc |
                 |                        | <--websocket-- bitstamp 
                 +------------------------+
```

## Project structure
```
. (grpc server impl)
├── protos (protobuf crate)
└── web-ui (add-on webapp ui)
```

## Config
Configuration environment variables (read on startup).

* `GRPC_PORT` Grpc server port. Default `7016`.
* `BINANCE_URL` Binance exchange base websocket url. Default `wss://stream.binance.com:9443`.
* `BITSTAMP_URL` Bitstamp exchange base websocket url. Default `wss://ws.bitstamp.net`.

## Test
Run a blackbox test scenario against mock binance & bitstamp ws services. See [tests/grpc.rs](./tests/grpc.rs).

```sh
cargo test
```

Note: Scenario is sufficent to test behaviour of grpc logic without additional unit tests.

## Run
Run the grpc server with 

```sh
cargo run --release
```

### Web UI
The merged live top 10 can be viewed using the _web-ui_ project. With the grpc server running run:

```sh
cargo run -p merged-order-book-web-ui --release
```

Open http://localhost:7000.

![](webui.png "Web UI")

## Implementation notes
* Only "etcbtc" is supported for simplicity. The service could be extended to support configurable / multiple.

* Exchange websocket streams are subscribed & confirmed on startup, failing will exit the app startup.
  This approach keeps things simple and rugged. It's also suitable for a scenario where there are many grpc
  clients at most times (so we always want to be subscribed to exchanges). Failing at startup works well with
  kubernetes since the service will never be reachable and will be restarted or fail loop with the old service still running.

  It would also be possible to lazily startup subscriptions, though this could introduce a class of errors after startup. Also kill subscriptions if there are no clients. This may make sense if the grpc service expects few clients, or supports many currencies some of which may have few clients.

* Exchange websockets will auto re-connect on close. However, there's no handling of custom re-connect messages or connection-open-but-not-working detection. This is left out to in the interests of simplicity and impl time.

* Note: The _service.proto_ definition is part of the example setup, rather than a choice.

* Proto generated structs are re-used inside the app despite not being the ideal data type. This makes the service simpler by avoiding and additional model type + conversion. An internal model could be slightly more efficient, e.g. avoiding `String exchange` in each `Level`.

* Prices & amounts are converted to f64 from the exchange string values. This could cause accuracy errors. However, since the protos enforce the use of double/f64 and no manipulation is happening, no additional inaccuracy will be caused. It would be more appropriate to use a decimal type with better guarantees than f64.

* Tracing / logging is absent. There are some `eprintln!` calls that could be trivially replaced with `tracing::info!` and tracing can integrated into websocket clients & grpc. This is left out to in the interests of simplicity and impl time.

* No metrics. A prometheus endpoint can provide metrics for the grpc server and websocket clients and perhaps track latency. This is left out to in the interests of simplicity and impl time.

* Dockerfile & kubernetes config are left out for simplicity.

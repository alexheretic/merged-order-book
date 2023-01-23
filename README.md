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
├── tests (scenario tests)
├── protos (protobuf service contracts)
└── grpc (grpc server impl)
```

## Config
Configuration environment variables (read on startup).

* `GRPC_PORT` Grpc server port. Default `7016`.
* `BINANCE_URL` Binance base websocket url. Default `wss://stream.binance.com:9443`.
* `BITSTAMP_URL` Binance base websocket url. Default `wss://ws.bitstamp.net`.

## Test
Run a blackbox test scenario against mock binance & bitstamp ws services. See [tests/grpc.rs](./tests/grpc.rs).

```sh
cargo test
```

Note: Scenario is sufficent to test behaviour of grpc logic without additional unit tests.

## Run
Run the grpc server with `cargo run -p merged-order-book-grpc`.

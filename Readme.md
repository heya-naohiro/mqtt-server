> [!WARNING]
> 🚧 This repository is for understanding the handling of MQTT and tokio, and experimental. It is a work in progress.

## build
```
cargo build
```

## Payload Storage
require cassandra

## option
```
mqtt server

USAGE:
    mqttserver [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a, --addr <IPADDR>                 server's address consist of port [default: 127.0.0.1:8883]
    -d, --db_addr <CASSANDRA IPADDR>    server's address consist of port [default: 127.0.0.1:9042]
    -c, --cert <FILEPATH>               server cert @ pem format [default: server.crt]
    -k, --key <FILEPATH>                server key @ pem format [default: private.key]
```

## Goal, not Goal
Not aiming for now: Implementing broker features, Communication between devices other than the host, adding protocols other than MQTT.

Future goals: Scaling out, performance measurement.

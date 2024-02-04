> [!WARNING]
> This repository is for understanding the handling of MQTT and tokio, and experimental. It is a work in progress.

## build
```
cargo build
```

## run
```
mqtt-server 0.0.0
Naohiro Heya
mqtt server

USAGE:
    mqttserver [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a, --addr <IPADDR>      server's address consist of port [default: 127.0.0.1:8883]
    -c, --cert <FILEPATH>    server cert @ pem format [default: server.crt]
    -k, --key <FILEPATH>     server cert @ pem format [default: private.key]
```
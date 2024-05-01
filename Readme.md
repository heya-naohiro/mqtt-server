> [!WARNING]
> 🚧 This repository is for understanding the handling of MQTT and tokio, and experimental. It is a work in progress.

## architecture

```mermaid
graph LR
    client["mqtt client"] <-- MQTT --> MQTTServer
    User <-- gRPC Publish/Streaming --> MQTTServer
    subgraph server
        direction BT 
        MQTTServer --> DB[(Database)]
    end
```

- The user can communicate with the server via gRPC and publish packets to the devices.

- The user can receive packets published by the devices through gRPC server streaming via the server.

- Pub/Sub communication between devices and the server via MQTT.

- The server utilizes tokio's green threads to concurrently handle communication with multiple devices.

- I am currently only implementing QoS0 / MQTTv3.1

## build
```
cargo build
```

## Payload Storage
If you specify the address of Cassandra, you can save the latest published payload to Cassandra.

## Usage
```
USAGE:
    mqtt-server [FLAGS] [OPTIONS]

FLAGS:
    -h, --help          Prints help information
    -n, --non-broker    server start in a mode where communication with clients occurs exclusively via gRPC, not through
                        an MQTT broker, and direct communication between clients is not possible.
        --non-tls       non tls mode
    -V, --version       Prints version information

OPTIONS:
    -a, --addr <IPADDR>                 server's address consist of port [default: 127.0.0.1:8883]
    -d, --db_addr <CASSANDRA IPADDR>    This is the port for Cassandra used to store the latest topics. If not
                                        specified, Cassandra will not be used. [default: ]
    -c, --cert <FILEPATH>               server cert @ pem format [default: server.crt]
    -k, --key <FILEPATH>                server key @ pem format [default: private.key]
```

## Performance
@ MacBook Pro (Retina, 15-inch, Mid 2015)

tool: [https://github.com/inovex/mqtt-stresser](https://github.com/inovex/mqtt-stresser)
```
[~/mqtt-stresser]$./mqtt-stresser -broker tcp://localhost:8883 -num-clients 1 -num-messages 50000 -rampup-delay 1s -rampup-size 10 -global-timeout 180s -timeout 20s -publisher-qos 0
1 worker started

# Configuration
Concurrent Clients: 1
Messages / Client:  50000

# Results
Published Messages: 50000 (100%)
Received Messages:  50000 (100%)
Completed:          1 (100%)
Errors:             0 (0%)

# Publishing Throughput
Fastest: 74755 msg/sec
Slowest: 74755 msg/sec
Median: 74755 msg/sec

  < 74755 msg/sec  100%

# Receiving Througput
Fastest: 119381 msg/sec
Slowest: 119381 msg/sec
Median: 119381 msg/sec

  < 119381 msg/sec  100%

```
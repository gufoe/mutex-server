# Mutex Server

This project provides a TCP server that will act as a cross-server mutex.
When clients disconnect or timeout, the mutexes are automatically released.

## Why you should not use it in production

- It uses busy waiting instead of real mutexes
- It uses one thread per connection
- There are no tests

## Example usage

```sh
./mutex-server --bind 127.0.0.1:9922
```

## TCP commands

You need to connect through TCP and send packets with the following contents.

# Lock a mutex

```json
{ "command": "Lock", "params": { "id": "123" } }
```

# Release a mutex

Just disconnect from the server or send this packet:

```json
{ "command": "Release", "params": { "id": "123" } }
```

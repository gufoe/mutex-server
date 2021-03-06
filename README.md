# Mutex Server
This project provides a TCP server that provides a cross-process and cross-node mutex.
When clients disconnect, the mutexes are automatically released.

## Why you should not use it in production
- It used busy waiting instead of real mutexes
- It uses one thread per connection
- There are no tests


## Example usage
```sh
./mutex-server --bind 127.0.0.1:54321
```

## TCP commands
You need to connect through TCP and send packets with the following contents.

# Lock a mutex
```json
{
    "command":"Lock",
    "params":{"id":"123"}
}
```


Release a mutex
```json
{
    "command":"Release",
    "params":{"id":"123"}
}
```

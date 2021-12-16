# Mutex Server
This project provides a TCP server that provides a cross-process and cross-node mutex.
When clients disconnect, the mutexes are automatically released.

## Why you should not use it in production
- It used busy waiting instead of real mutexes
- It uses one thread per connection
- There are no tests

## TCP commands

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

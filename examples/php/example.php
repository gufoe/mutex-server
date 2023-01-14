<?php

class TcpMutexClient
{
    public \Socket $socket;
    function __construct(public string $host, public int $port)
    {
        $socket = socket_create(AF_INET, SOCK_STREAM, SOL_TCP);
        if (!$socket) throw new \Error("Failed to create socket");

        socket_connect($socket, $host, $port)
            or throw new \Error("Failed to connect to mutex server");
        $this->socket = $socket;
    }

    function write($payload)
    {
        socket_write($this->socket, $payload, strlen($payload)) or throw new \Error("Cannot send packet");
    }

    function readJson()
    {
        $response = $this->read();
        return json_decode($response);
    }
    function read()
    {
        $buffer = '';
        while (true) {
            $chunk = socket_read($this->socket, 1, PHP_NORMAL_READ);
            if ($chunk === "\n") break;
            $buffer .= $chunk;
        }
        return $buffer;
    }

    function lock(string $id, int $timeout = null)
    {
        return new TcpMutex($this, $id, $timeout);
    }
}

class TcpMutex
{
    function __construct(public TcpMutexClient $client, public string $id, public ?int $timeout = null)
    {
        $this->client->write(json_encode([
            'command' => 'Lock',
            'params' => ['id' => $this->id, 'timeout_ms' => $this->timeout],
        ]));
        $response = $this->client->readJson();
        if ($response->command != 'LockResponse') {
            throw new \Error("Invalid server response");
        }
        if ($response->params->id != $this->id) {
            throw new \Error("Invalid server response");
        }
        if (!$response->params->success) {
            throw new \Error("Mutex not locked");
        }
    }

    function release()
    {
        $this->client->write(json_encode([
            'command' => 'Release',
            'params' => ['id' => $this->id],
        ]));
        $response = $this->client->readJson();
        if ($response->command != 'ReleaseResponse') {
            throw new \Error("Invalid server response");
        }
        if ($response->params->id != $this->id) {
            throw new \Error("Invalid server response");
        }
        if (!$response->params->success) {
            throw new \Error("Mutex not locked");
        }
    }
}

$client = new TcpMutexClient('127.0.0.1', 9922);
while (true) {


    echo "Acquiring mutex...\n";
    $mutex = $client->lock(10, 1000);
    echo "Mutex acquired\n";

    echo "Sleep 5 seconds...\n";
    sleep(5);

    echo "Releasing mutex...\n";
    $mutex->release();
    echo "Mutex released\n";
}

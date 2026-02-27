<?php

namespace App\Services;

class UserRepository
{
    private $connection;

    public function __construct($connection)
    {
        $this->connection = $connection;
    }

    public function findAll()
    {
        return $this->connection->query("SELECT * FROM users");
    }

    public function findById($id)
    {
        return $this->connection->query("SELECT * FROM users WHERE id = ?", [$id]);
    }

    public function delete($id)
    {
        $this->connection->execute("DELETE FROM users WHERE id = ?", [$id]);
    }
}

class OrderRepository
{
    private $connection;

    public function findAll()
    {
        return $this->connection->query("SELECT * FROM orders");
    }

    public function create($data)
    {
        $this->connection->execute("INSERT INTO orders VALUES (?)", [$data]);
    }
}

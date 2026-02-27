package com.example;

import java.util.List;
import java.util.ArrayList;

public class UserService {
    private String serviceName;

    public UserService(String name) {
        this.serviceName = name;
    }

    public List<String> getUsers() {
        return new ArrayList<>();
    }

    public String findUser(String id) {
        return "user-" + id;
    }

    public void deleteUser(String id) {
        // delete user
    }
}

class OrderService {
    private int maxOrders;

    public OrderService(int max) {
        this.maxOrders = max;
    }

    public List<String> getUsers() {
        // Different from UserService.getUsers
        return new ArrayList<>();
    }

    public void processOrder(String orderId) {
        System.out.println("Processing " + orderId);
    }
}

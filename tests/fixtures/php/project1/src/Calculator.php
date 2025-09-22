<?php

namespace TestProject;

use TestProject\Utils;

/**
 * Calculator class with core business logic
 * Provides complex calculations using utility functions for LSP testing
 */
class Calculator
{
    /**
     * Calculate performs a complex calculation using utility functions
     * This function should show both incoming calls (from main, processNumbers, BusinessLogic)
     * and outgoing calls (to add, multiply, subtract)
     *
     * @param int $a First operand
     * @param int $b Second operand
     * @return int Calculated result
     */
    public function calculate(int $a, int $b): int
    {
        $sum = Utils::add($a, $b);        // Outgoing call to add
        $result = Utils::multiply($sum, 2); // Outgoing call to multiply

        // Additional logic for testing
        if ($result > 50) {
            $result = Utils::subtract($result, 10); // Outgoing call to subtract
        }

        return $result;
    }

    /**
     * Advanced calculation that also uses the calculate function
     * This creates another pathway in the call hierarchy
     *
     * @param array $values Array of values to process
     * @return int Advanced result
     */
    public function advancedCalculation(array $values): int
    {
        $total = 0;
        foreach ($values as $value) {
            $total += $this->calculate($value, 1); // Incoming call to calculate
        }
        return $total;
    }

    /**
     * Batch processing method that demonstrates multiple call patterns
     *
     * @param array $data Array of input data pairs
     * @return array Processed results
     */
    public function processArray(array $data): array
    {
        return array_map(function($item) {
            $intermediate = Utils::add($item, 5); // Outgoing call to add
            return $this->calculate($intermediate, 2); // Outgoing call to calculate
        }, $data);
    }
}

/**
 * Business logic interface for testing interface call hierarchies
 */
interface IBusinessLogic
{
    /**
     * Process a value using business logic
     *
     * @param int $value Input value
     * @return int Processed result
     */
    public function processValue(int $value): int;
}

/**
 * Complex business logic class implementing IBusinessLogic
 * Demonstrates class-based call hierarchies and polymorphism
 */
class BusinessLogic implements IBusinessLogic
{
    private int $multiplier;
    private Calculator $calculator;

    /**
     * Constructor
     *
     * @param int $multiplier Multiplier value for calculations
     */
    public function __construct(int $multiplier)
    {
        $this->multiplier = $multiplier;
        $this->calculator = new Calculator();
    }

    /**
     * Process a value using internal logic
     *
     * @param int $value Input value
     * @return int Processed result
     */
    public function processValue(int $value): int
    {
        return $this->calculator->calculate($value, $this->multiplier); // Another incoming call to calculate
    }

    /**
     * Complex processing that chains multiple calls
     *
     * @param array $data Array of input data
     * @return array Processed results
     */
    public function processData(array $data): array
    {
        $results = [];
        foreach ($data as $item) {
            $intermediate = Utils::add($item, 5); // Outgoing call to add
            $results[] = $this->calculator->calculate($intermediate, 2); // Outgoing call to calculate
        }
        return $results;
    }

    /**
     * Helper method that demonstrates private method calls
     *
     * @param int $value Input value
     * @return int Processed result
     */
    public function processWithValidation(int $value): int
    {
        if ($this->validateInput($value)) {
            return $this->processValue($value);
        }
        return 0;
    }

    /**
     * Private validation method
     *
     * @param int $value Value to validate
     * @return bool True if valid
     */
    private function validateInput(int $value): bool
    {
        return $value >= 0 && $value <= 1000;
    }
}
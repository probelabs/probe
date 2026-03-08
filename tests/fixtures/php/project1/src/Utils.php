<?php

namespace TestProject;

/**
 * Utility functions class for mathematical operations and helpers
 * This class provides static methods that should show incoming calls from various parts of the application
 */
class Utils
{
    /**
     * Add two numbers together
     * This function should show incoming calls from calculate and other functions
     *
     * @param int $x First number
     * @param int $y Second number
     * @return int Sum of x and y
     */
    public static function add(int $x, int $y): int
    {
        return $x + $y;
    }

    /**
     * Multiply two numbers
     * This function should show incoming calls from calculate and other functions
     *
     * @param int $x First number
     * @param int $y Second number
     * @return int Product of x and y
     */
    public static function multiply(int $x, int $y): int
    {
        return $x * $y;
    }

    /**
     * Subtract two numbers
     * This function should show incoming calls from calculate
     *
     * @param int $x First number
     * @param int $y Second number
     * @return int Difference of x and y
     */
    public static function subtract(int $x, int $y): int
    {
        return $x - $y;
    }

    /**
     * Divide two numbers with safety check
     * This function might not have incoming calls in our test
     *
     * @param int $x Dividend
     * @param int $y Divisor
     * @return float Quotient
     * @throws \InvalidArgumentException When divisor is zero
     */
    public static function divide(int $x, int $y): float
    {
        if ($y === 0) {
            throw new \InvalidArgumentException("Division by zero");
        }
        return $x / $y;
    }

    /**
     * Utility helper that demonstrates chained function calls
     *
     * @param int $a First input
     * @param int $b Second input
     * @return int Computed result
     */
    public static function utilityHelper(int $a, int $b): int
    {
        $temp = self::add($a, $b);      // Outgoing call to add
        return self::multiply($temp, 3); // Outgoing call to multiply
    }

    /**
     * Format a number for display with thousand separators
     *
     * @param int $number Number to format
     * @return string Formatted number string
     */
    public static function formatNumber(int $number): string
    {
        return number_format($number);
    }

    /**
     * Validate input values for processing
     *
     * @param int $value Value to validate
     * @return bool True if value is valid for processing
     */
    public static function validateInput(int $value): bool
    {
        return $value >= 0 && $value <= 10000;
    }

    /**
     * Calculate percentage of a value
     *
     * @param int $value Base value
     * @param float $percentage Percentage to calculate
     * @return float Calculated percentage value
     */
    public static function calculatePercentage(int $value, float $percentage): float
    {
        $multiplier = $percentage / 100;
        return self::multiply($value, intval($multiplier * 100)) / 100; // Outgoing call to multiply
    }

    /**
     * Round a number to specified decimal places
     *
     * @param float $value Value to round
     * @param int $decimals Number of decimal places
     * @return float Rounded value
     */
    public static function roundNumber(float $value, int $decimals = 2): float
    {
        return round($value, $decimals);
    }
}

/**
 * Math utilities class for advanced mathematical operations
 * Demonstrates namespaced utility functions and class-based organization
 */
class MathUtils
{
    /**
     * Power function that uses multiply internally
     *
     * @param int $base Base number
     * @param int $exponent Exponent (must be positive integer)
     * @return int Result of base^exponent
     */
    public static function power(int $base, int $exponent): int
    {
        if ($exponent === 0) return 1;
        if ($exponent === 1) return $base;

        $result = $base;
        for ($i = 1; $i < $exponent; $i++) {
            $result = Utils::multiply($result, $base); // Outgoing call to multiply
        }
        return $result;
    }

    /**
     * Square function
     *
     * @param int $x Input number
     * @return int Square of x
     */
    public static function square(int $x): int
    {
        return Utils::multiply($x, $x); // Outgoing call to multiply
    }

    /**
     * Factorial function with recursive implementation
     *
     * @param int $n Input number
     * @return int Factorial of n
     */
    public static function factorial(int $n): int
    {
        if ($n <= 1) return 1;
        return Utils::multiply($n, self::factorial($n - 1)); // Outgoing call to multiply and recursive call
    }

    /**
     * Greatest common divisor using Euclidean algorithm
     *
     * @param int $a First number
     * @param int $b Second number
     * @return int GCD of a and b
     */
    public static function gcd(int $a, int $b): int
    {
        while ($b !== 0) {
            $temp = $b;
            $b = $a % $b;
            $a = $temp;
        }
        return $a;
    }

    /**
     * Least common multiple using GCD
     *
     * @param int $a First number
     * @param int $b Second number
     * @return int LCM of a and b
     */
    public static function lcm(int $a, int $b): int
    {
        $gcd = self::gcd($a, $b); // Outgoing call to gcd
        return Utils::divide(Utils::multiply($a, $b), $gcd); // Outgoing calls to multiply and divide
    }
}

/**
 * String utilities for text processing
 * Demonstrates different utility categories and their interactions
 */
class StringUtils
{
    /**
     * Convert number to words (simple implementation)
     *
     * @param int $number Number to convert
     * @return string Number in words
     */
    public static function numberToWords(int $number): string
    {
        $words = [
            0 => 'zero', 1 => 'one', 2 => 'two', 3 => 'three', 4 => 'four',
            5 => 'five', 6 => 'six', 7 => 'seven', 8 => 'eight', 9 => 'nine'
        ];

        if ($number < 10) {
            return $words[$number] ?? 'unknown';
        }

        return 'number too large';
    }

    /**
     * Pad number with leading zeros
     *
     * @param int $number Number to pad
     * @param int $length Total length after padding
     * @return string Padded number string
     */
    public static function padNumber(int $number, int $length): string
    {
        return str_pad((string)$number, $length, '0', STR_PAD_LEFT);
    }

    /**
     * Format number with custom formatting
     *
     * @param int $number Number to format
     * @param string $prefix Prefix to add
     * @param string $suffix Suffix to add
     * @return string Formatted string
     */
    public static function formatWithPrefixSuffix(int $number, string $prefix = '', string $suffix = ''): string
    {
        $formatted = Utils::formatNumber($number); // Outgoing call to formatNumber
        return $prefix . $formatted . $suffix;
    }
}
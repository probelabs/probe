<?php

namespace TestProject;

use TestProject\Calculator;
use TestProject\BusinessLogic;
use TestProject\Utils;

/**
 * Main application class demonstrating entry points and process flows
 * This class creates the primary call hierarchy for LSP testing
 */
class Main
{
    private Calculator $calculator;
    private BusinessLogic $businessLogic;

    /**
     * Constructor initializing dependencies
     */
    public function __construct()
    {
        $this->calculator = new Calculator();
        $this->businessLogic = new BusinessLogic(3);
    }

    /**
     * Main entry point for the application
     * This method should show outgoing calls to various other methods
     *
     * @return void
     */
    public function main(): void
    {
        echo "Starting PHP test application\n";

        // Direct calculation call
        $result1 = $this->calculator->calculate(10, 5); // Outgoing call to calculate
        echo "Direct calculation result: $result1\n";

        // Processing numbers through helper method
        $numbers = [1, 2, 3, 4, 5];
        $results = $this->processNumbers($numbers); // Outgoing call to processNumbers
        echo "Processed numbers: " . implode(', ', $results) . "\n";

        // Business logic processing
        $businessResult = $this->businessLogic->processValue(20); // Outgoing call to processValue
        echo "Business logic result: $businessResult\n";

        // Utility function demonstration
        $formatted = Utils::formatNumber($businessResult); // Outgoing call to formatNumber
        echo "Formatted result: $formatted\n";

        // Complex processing workflow
        $this->runComplexWorkflow(); // Outgoing call to runComplexWorkflow
    }

    /**
     * Process multiple numbers using various calculation methods
     * This method should show incoming calls from main and outgoing calls to calculate functions
     *
     * @param array $numbers Array of numbers to process
     * @return array Processed results
     */
    public function processNumbers(array $numbers): array
    {
        $results = [];

        foreach ($numbers as $number) {
            // Validate input first
            if (Utils::validateInput($number)) { // Outgoing call to validateInput
                // Process through calculator
                $calculated = $this->calculator->calculate($number, 2); // Outgoing call to calculate

                // Process through business logic
                $businessProcessed = $this->businessLogic->processValue($calculated); // Outgoing call to processValue

                $results[] = $businessProcessed;
            }
        }

        return $results;
    }

    /**
     * Complex workflow demonstrating multiple call chains
     * This creates a deeper call hierarchy for testing
     *
     * @return void
     */
    public function runComplexWorkflow(): void
    {
        echo "Running complex workflow\n";

        // Step 1: Prepare data
        $data = $this->prepareData(); // Outgoing call to prepareData

        // Step 2: Advanced calculation
        $advancedResult = $this->calculator->advancedCalculation($data); // Outgoing call to advancedCalculation
        echo "Advanced calculation result: $advancedResult\n";

        // Step 3: Business processing
        $businessResults = $this->businessLogic->processData($data); // Outgoing call to processData
        echo "Business processing results: " . implode(', ', $businessResults) . "\n";

        // Step 4: Utility operations
        $this->performUtilityOperations($businessResults); // Outgoing call to performUtilityOperations
    }

    /**
     * Prepare test data for processing
     * This method demonstrates data preparation patterns
     *
     * @return array Prepared data array
     */
    private function prepareData(): array
    {
        $baseData = [10, 20, 30];
        $preparedData = [];

        foreach ($baseData as $value) {
            $modified = Utils::add($value, 5); // Outgoing call to add
            $preparedData[] = $modified;
        }

        return $preparedData;
    }

    /**
     * Perform various utility operations for demonstration
     * This method chains multiple utility function calls
     *
     * @param array $values Input values to process
     * @return void
     */
    private function performUtilityOperations(array $values): void
    {
        echo "Performing utility operations\n";

        foreach ($values as $value) {
            // Chain multiple utility calls
            $doubled = Utils::multiply($value, 2); // Outgoing call to multiply
            $formatted = Utils::formatNumber($doubled); // Outgoing call to formatNumber

            echo "Processed value: $formatted\n";
        }

        // Demonstrate helper utility
        $helperResult = Utils::utilityHelper(10, 20); // Outgoing call to utilityHelper
        echo "Helper result: $helperResult\n";
    }

    /**
     * Static method for testing static call hierarchies
     *
     * @param int $value Input value
     * @return int Processed value
     */
    public static function staticProcessor(int $value): int
    {
        $result = Utils::multiply($value, 3); // Outgoing call to multiply
        return Utils::add($result, 1); // Outgoing call to add
    }
}

/**
 * Application runner class for testing different entry points
 */
class ApplicationRunner
{
    /**
     * Run the main application
     *
     * @return void
     */
    public static function run(): void
    {
        $app = new Main();
        $app->main(); // Outgoing call to main

        // Additional processing
        $staticResult = Main::staticProcessor(15); // Outgoing call to staticProcessor
        echo "Static processor result: $staticResult\n";
    }
}
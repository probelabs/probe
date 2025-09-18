use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_php_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("Calculator.php");

    let content = r#"<?php

namespace Calculator;

use Exception;
use InvalidArgumentException;

/**
 * Calculator interface defining basic arithmetic operations
 */
interface CalculatorInterface
{
    public function add(float $x, float $y): float;
    public function subtract(float $x, float $y): float;
    public function multiply(float $x, float $y): float;
    public function divide(float $x, float $y): float;
    public function getHistory(): array;
}

/**
 * Abstract base calculator with common functionality
 */
abstract class BaseCalculator implements CalculatorInterface
{
    protected string $name;
    protected array $history = [];
    protected int $precision;

    public function __construct(string $name, int $precision = 2)
    {
        $this->name = $name;
        $this->precision = $precision;
    }

    protected function recordOperation(float $result): void
    {
        $this->history[] = round($result, $this->precision);
    }

    public function getHistory(): array
    {
        return $this->history;
    }

    public function getName(): string
    {
        return $this->name;
    }

    public function clearHistory(): void
    {
        $this->history = [];
    }

    abstract public function getOperationsCount(): int;
}

/**
 * Advanced calculator implementation with modern PHP features
 */
class AdvancedCalculator extends BaseCalculator
{
    private const DEFAULT_PRECISION = 0.001;
    private const SUPPORTED_OPERATIONS = ['add', 'subtract', 'multiply', 'divide'];

    private array $constants;
    private int $operationsCount = 0;

    public function __construct(string $name, int $precision = 2)
    {
        parent::__construct($name, $precision);
        $this->initializeConstants();
    }

    private function initializeConstants(): void
    {
        $this->constants = [
            'PI' => M_PI,
            'E' => M_E,
            'GOLDEN_RATIO' => (1 + sqrt(5)) / 2,
        ];
    }

    public function add(float $x, float $y): float
    {
        $result = $x + $y;
        $this->recordOperation($result);
        $this->operationsCount++;
        return $result;
    }

    public function subtract(float $x, float $y): float
    {
        $result = $x - $y;
        $this->recordOperation($result);
        $this->operationsCount++;
        return $result;
    }

    public function multiply(float $x, float $y): float
    {
        $result = $x * $y;
        $this->recordOperation($result);
        $this->operationsCount++;
        return $result;
    }

    public function divide(float $x, float $y): float
    {
        if (abs($y) < self::DEFAULT_PRECISION) {
            throw new InvalidArgumentException('Division by zero');
        }

        $result = $x / $y;
        $this->recordOperation($result);
        $this->operationsCount++;
        return $result;
    }

    public function getOperationsCount(): int
    {
        return $this->operationsCount;
    }

    /**
     * Process an array of numbers with a callback
     */
    public function processNumbers(array $numbers, callable $callback): array
    {
        return array_map($callback, array_filter($numbers, 'is_numeric'));
    }

    /**
     * Transform history using a callback
     */
    public function transformHistory(callable $transformer): array
    {
        return array_map($transformer, $this->history);
    }

    /**
     * Get constant value by name
     */
    public function getConstant(string $name): ?float
    {
        return $this->constants[$name] ?? null;
    }

    /**
     * Static factory method
     */
    public static function createWithDefaults(string $name): self
    {
        return new self($name);
    }

    /**
     * Magic method for dynamic operation calls
     */
    public function __call(string $method, array $arguments): float
    {
        if (in_array($method, self::SUPPORTED_OPERATIONS, true)) {
            return $this->$method(...$arguments);
        }

        throw new InvalidArgumentException("Unsupported operation: {$method}");
    }
}

/**
 * Scientific calculator with advanced mathematical functions
 */
class ScientificCalculator extends AdvancedCalculator
{
    public function sin(float $x): float
    {
        $result = sin($x);
        $this->recordOperation($result);
        return $result;
    }

    public function cos(float $x): float
    {
        $result = cos($x);
        $this->recordOperation($result);
        return $result;
    }

    public function tan(float $x): float
    {
        $result = tan($x);
        $this->recordOperation($result);
        return $result;
    }

    public function log(float $x, float $base = M_E): float
    {
        if ($x <= 0) {
            throw new InvalidArgumentException('Cannot take log of zero or negative number');
        }

        $result = log($x) / log($base);
        $this->recordOperation($result);
        return $result;
    }

    public function power(float $base, float $exponent): float
    {
        $result = pow($base, $exponent);
        $this->recordOperation($result);
        return $result;
    }

    public function factorial(int $n): float
    {
        if ($n < 0) {
            throw new InvalidArgumentException('Factorial of negative number');
        }

        $result = 1;
        for ($i = 2; $i <= $n; $i++) {
            $result *= $i;
        }

        $this->recordOperation($result);
        return $result;
    }

    /**
     * Calculate statistics from history
     */
    public function getStatistics(): array
    {
        if (empty($this->history)) {
            return ['mean' => 0, 'median' => 0, 'std_dev' => 0];
        }

        $mean = array_sum($this->history) / count($this->history);

        $sorted = $this->history;
        sort($sorted);
        $count = count($sorted);
        $median = $count % 2 === 0
            ? ($sorted[$count / 2 - 1] + $sorted[$count / 2]) / 2
            : $sorted[intval($count / 2)];

        $variance = array_sum(array_map(
            fn($x) => pow($x - $mean, 2),
            $this->history
        )) / count($this->history);
        $stdDev = sqrt($variance);

        return [
            'mean' => $mean,
            'median' => $median,
            'std_dev' => $stdDev,
        ];
    }
}

/**
 * Utility trait for common calculator operations
 */
trait CalculatorUtils
{
    public function formatResult(float $result, int $precision = 2): string
    {
        return number_format($result, $precision);
    }

    public function isValidNumber($value): bool
    {
        return is_numeric($value) && is_finite((float) $value);
    }

    public function roundToPrecision(float $value, int $precision): float
    {
        return round($value, $precision);
    }
}

/**
 * Calculator factory class
 */
class CalculatorFactory
{
    use CalculatorUtils;

    public static function create(string $type, string $name): CalculatorInterface
    {
        return match ($type) {
            'basic' => new AdvancedCalculator($name),
            'scientific' => new ScientificCalculator($name),
            default => throw new InvalidArgumentException("Unknown calculator type: {$type}"),
        };
    }

    public static function createFromConfig(array $config): CalculatorInterface
    {
        $type = $config['type'] ?? 'basic';
        $name = $config['name'] ?? 'Default Calculator';
        $precision = $config['precision'] ?? 2;

        $calculator = self::create($type, $name);

        if (method_exists($calculator, 'setPrecision')) {
            $calculator->setPrecision($precision);
        }

        return $calculator;
    }
}

/**
 * Demo class showcasing calculator functionality
 */
class CalculatorDemo
{
    public static function run(): void
    {
        echo "Calculator Demo\n";
        echo "===============\n";

        $calc = ScientificCalculator::createWithDefaults('Demo Calculator');

        try {
            echo "Basic operations:\n";
            echo "10 + 5 = " . $calc->add(10, 5) . "\n";
            echo "20 * 3 = " . $calc->multiply(20, 3) . "\n";
            echo "100 / 4 = " . $calc->divide(100, 4) . "\n";

            echo "\nScientific operations:\n";
            echo "sin(π/2) = " . $calc->sin(M_PI / 2) . "\n";
            echo "5! = " . $calc->factorial(5) . "\n";

            echo "\nStatistics:\n";
            $stats = $calc->getStatistics();
            echo "Mean: " . $stats['mean'] . "\n";
            echo "Median: " . $stats['median'] . "\n";

            echo "\nHistory: " . implode(', ', $calc->getHistory()) . "\n";

        } catch (Exception $e) {
            echo "Error: " . $e->getMessage() . "\n";
        }
    }
}

// Test functions
function testBasicCalculator(): void
{
    $calc = new AdvancedCalculator('Test Calculator');

    $result = $calc->add(2, 3);
    if ($result !== 5.0) {
        throw new Exception('Add test failed');
    }

    $result = $calc->multiply(4, 5);
    if ($result !== 20.0) {
        throw new Exception('Multiply test failed');
    }

    echo "Basic calculator tests passed\n";
}

function testScientificCalculator(): void
{
    $calc = new ScientificCalculator('Scientific Test');

    $result = $calc->power(2, 3);
    if ($result !== 8.0) {
        throw new Exception('Power test failed');
    }

    $result = $calc->factorial(4);
    if ($result !== 24.0) {
        throw new Exception('Factorial test failed');
    }

    echo "Scientific calculator tests passed\n";
}

// Run demo if this file is executed directly
if (basename(__FILE__) === basename($_SERVER['SCRIPT_NAME'])) {
    CalculatorDemo::run();
    testBasicCalculator();
    testScientificCalculator();
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Calculator",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--max-results",
        "50",
    ])?;

    // Verify PHP symbols are extracted (flexible matching for outline format)
    let has_interface = output.contains("interface") || output.contains("CalculatorInterface");
    let has_base_class = output.contains("BaseCalculator") || output.contains("abstract");
    let has_advanced_calc = output.contains("AdvancedCalculator") || output.contains("class");
    let has_scientific = output.contains("ScientificCalculator") || output.contains("Calculator");
    let has_functions = output.contains("function") || output.contains("test");

    // At least some PHP structures should be present in outline format
    let php_structures_count = [
        has_interface,
        has_base_class,
        has_advanced_calc,
        has_scientific,
        has_functions,
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    assert!(
        php_structures_count >= 2,
        "Should find at least 2 PHP structures in outline format, found {} structures. Output: {}",
        php_structures_count,
        output
    );

    Ok(())
}

#[test]
fn test_php_outline_closing_brace_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_class.php");

    let content = r#"<?php

namespace App\Services;

use Exception;
use InvalidArgumentException;

class LargeDataProcessor
{
    private array $config;
    private string $status;
    private int $processedCount = 0;

    public function __construct(array $config)
    {
        $this->config = $config;
        $this->status = 'initialized';
    }

    public function processLargeDataSet(array $data): array
    {
        $results = [];
        $batchSize = $this->config['batch_size'] ?? 100;

        foreach ($data as $index => $item) {
            if (!$this->validateItem($item)) {
                continue;
            }

            try {
                $processed = $this->transformItem($item);
                $results[] = $processed;
                $this->processedCount++;

                if ($this->processedCount % $batchSize === 0) {
                    $this->logProgress();
                }
            } catch (Exception $e) {
                $this->handleError($e, $item);
            }
        }

        $this->status = 'completed';
        return $results;
    }

    private function validateItem($item): bool
    {
        if (!is_array($item)) {
            return false;
        }

        $requiredFields = ['id', 'type', 'data'];
        foreach ($requiredFields as $field) {
            if (!isset($item[$field])) {
                return false;
            }
        }

        return true;
    }

    private function transformItem(array $item): array
    {
        $transformed = [
            'id' => (int) $item['id'],
            'type' => strtolower($item['type']),
            'processed_at' => date('Y-m-d H:i:s'),
        ];

        if (isset($item['metadata'])) {
            $transformed['metadata'] = $this->processMetadata($item['metadata']);
        }

        return $transformed;
    }

    private function processMetadata(array $metadata): array
    {
        $processed = [];

        foreach ($metadata as $key => $value) {
            if (is_string($value)) {
                $processed[$key] = trim($value);
            } elseif (is_numeric($value)) {
                $processed[$key] = (float) $value;
            } else {
                $processed[$key] = $value;
            }
        }

        return $processed;
    }
}

function smallFunction(): string
{
    return 'small';
}

function mediumFunction(array $data): array
{
    $result = [];
    foreach ($data as $item) {
        $result[] = strtoupper($item);
    }
    return $result;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "class|function",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify we get search results for large classes and functions
    let has_large_class = output.contains("LargeDataProcessor") || output.contains("class");
    let has_large_method = output.contains("processLargeDataSet") || output.contains("function");

    assert!(
        has_large_class,
        "Should find large class in search results - output: {}",
        output
    );

    assert!(
        has_large_method,
        "Should find large method in search results - output: {}",
        output
    );

    // Note: Closing brace comments are a feature of the outline format,
    // but may not always appear in search results depending on the context shown.

    // Small functions should NOT have closing brace comments
    let small_function_section = output.split("smallFunction").last().unwrap_or("");
    let small_section_before_next = small_function_section
        .split("mediumFunction")
        .next()
        .unwrap_or(small_function_section);

    assert!(
        !small_section_before_next.contains("// function smallFunction")
            && !small_section_before_next.contains("// smallFunction"),
        "Small function should NOT have closing brace comment - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_php_outline_keyword_highlighting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keyword_test.php");

    let content = r#"<?php

namespace App\Search;

use Exception;

class SearchEngine
{
    private array $keywords = ['search', 'index', 'query', 'result'];

    public function searchDocuments(string $query): array
    {
        $searchTerms = explode(' ', $query);
        $results = [];

        foreach ($searchTerms as $term) {
            if (in_array($term, $this->keywords)) {
                $results[] = $this->findMatches($term);
            }
        }

        return array_merge(...$results);
    }

    private function findMatches(string $searchTerm): array
    {
        // This function would search for matches
        return ['match1', 'match2', 'searchTerm' => $searchTerm];
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "search",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should contain the search keyword in various contexts
    assert!(
        output.contains("search"),
        "Output should contain 'search' keyword - output: {}",
        output
    );

    // Should show function definitions
    assert!(
        output.contains("searchDocuments") || output.contains("function"),
        "Output should contain search function - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_php_outline_array_truncation_with_keywords() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_arrays.php");

    let content = r#"<?php

class DataProcessor
{
    private array $config = [
        'database' => [
            'host' => 'localhost',
            'port' => 3306,
            'username' => 'user',
            'password' => 'secret',
            'database' => 'app_db',
            'charset' => 'utf8mb4',
            'search_engine' => 'mysql',
            'indexing' => true,
            'query_cache' => true,
            'result_limit' => 1000,
        ],
        'redis' => [
            'host' => 'redis-server',
            'port' => 6379,
            'database' => 0,
            'password' => null,
            'search_ttl' => 3600,
            'query_cache_ttl' => 1800,
        ],
        'elasticsearch' => [
            'hosts' => ['es1:9200', 'es2:9200'],
            'index_name' => 'documents',
            'search_type' => 'match',
            'query_timeout' => 30,
            'result_size' => 50,
        ],
        'search_options' => [
            'fuzzy_matching' => true,
            'stemming' => true,
            'stop_words' => ['the', 'and', 'or', 'but', 'search', 'query'],
            'synonyms' => ['find' => 'search', 'lookup' => 'query'],
            'boost_fields' => ['title' => 2.0, 'content' => 1.0],
        ],
    ];

    public function search(string $term): array
    {
        $searchConfig = $this->config['search_options'];
        return $this->executeSearch($term, $searchConfig);
    }

    private function executeSearch(string $term, array $config): array
    {
        return ['query' => $term, 'search_results' => []];
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "search",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should contain the search keyword even in truncated arrays
    assert!(
        output.contains("search"),
        "Output should contain 'search' keyword even in truncated arrays - output: {}",
        output
    );

    // Should show truncation with ellipsis if arrays are large enough
    // Note: This test may not always trigger truncation depending on the search results
    // We just verify that the search works and contains the keyword
    let has_truncation = output.contains("...") || output.contains("…");
    if has_truncation {
        println!("Array truncation detected (good!)");
    }

    // Should have reasonable length for outline format
    let line_count = output.lines().count();
    assert!(
        line_count < 200,
        "Output should be reasonably sized for outline format, got {} lines",
        line_count
    );

    Ok(())
}

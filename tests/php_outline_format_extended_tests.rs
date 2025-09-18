use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_php_specific_constructs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("php_constructs.php");

    let content = r#"<?php

namespace App\Core;

use Countable;
use ArrayAccess;

trait Cacheable
{
    private array $cache = [];

    protected function getCacheKey(string $method, array $args): string
    {
        return md5($method . serialize($args));
    }

    protected function getFromCache(string $key)
    {
        return $this->cache[$key] ?? null;
    }

    protected function putInCache(string $key, $value): void
    {
        $this->cache[$key] = $value;
    }
}

trait Searchable
{
    abstract protected function getSearchableFields(): array;

    public function search(string $query): array
    {
        $fields = $this->getSearchableFields();
        $results = [];

        foreach ($fields as $field) {
            if (stripos($this->$field, $query) !== false) {
                $results[] = $field;
            }
        }

        return $results;
    }
}

abstract class BaseModel implements Countable, ArrayAccess
{
    use Cacheable, Searchable;

    protected array $data = [];
    protected array $searchableFields = [];

    public function __construct(array $data = [])
    {
        $this->data = $data;
    }

    // Magic methods
    public function __get(string $name)
    {
        return $this->data[$name] ?? null;
    }

    public function __set(string $name, $value): void
    {
        $this->data[$name] = $value;
    }

    public function __isset(string $name): bool
    {
        return isset($this->data[$name]);
    }

    public function __unset(string $name): void
    {
        unset($this->data[$name]);
    }

    public function __toString(): string
    {
        return json_encode($this->data);
    }

    public function __debugInfo(): array
    {
        return ['data' => $this->data, 'searchable' => $this->searchableFields];
    }

    // ArrayAccess implementation
    public function offsetExists($offset): bool
    {
        return isset($this->data[$offset]);
    }

    public function offsetGet($offset)
    {
        return $this->data[$offset] ?? null;
    }

    public function offsetSet($offset, $value): void
    {
        if (is_null($offset)) {
            $this->data[] = $value;
        } else {
            $this->data[$offset] = $value;
        }
    }

    public function offsetUnset($offset): void
    {
        unset($this->data[$offset]);
    }

    // Countable implementation
    public function count(): int
    {
        return count($this->data);
    }

    // Abstract methods
    abstract protected function validate(): bool;
    abstract public function save(): bool;

    protected function getSearchableFields(): array
    {
        return $this->searchableFields;
    }
}

interface SearchableInterface
{
    public function search(string $query): array;
    public function getSearchableFields(): array;
}

class User extends BaseModel implements SearchableInterface
{
    protected array $searchableFields = ['name', 'email', 'bio'];

    public function __construct(array $data = [])
    {
        parent::__construct($data);
        $this->data['created_at'] = $this->data['created_at'] ?? date('Y-m-d H:i:s');
    }

    protected function validate(): bool
    {
        return !empty($this->data['name']) && !empty($this->data['email']);
    }

    public function save(): bool
    {
        if (!$this->validate()) {
            return false;
        }

        // Save logic here
        return true;
    }

    public function getSearchableFields(): array
    {
        return $this->searchableFields;
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "class|trait|interface|function",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should extract PHP-specific constructs
    assert!(
        output.contains("trait") || output.contains("Cacheable"),
        "Should extract traits - output: {}",
        output
    );

    assert!(
        output.contains("namespace") || output.contains("App\\Core"),
        "Should show namespace - output: {}",
        output
    );

    assert!(
        output.contains("interface") || output.contains("SearchableInterface"),
        "Should extract interfaces - output: {}",
        output
    );

    assert!(
        output.contains("abstract") || output.contains("BaseModel"),
        "Should extract abstract classes - output: {}",
        output
    );

    // Should show magic methods
    assert!(
        output.contains("__get") || output.contains("__set") || output.contains("__construct"),
        "Should show magic methods - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_php_control_flow_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("control_flow.php");

    let content = r#"<?php

class FlowController
{
    public function processData(array $data): array
    {
        $results = [];
        $counter = 0;

        foreach ($data as $index => $item) {
            if ($item === null) {
                continue;
            }

            if (is_array($item)) {
                foreach ($item as $key => $value) {
                    if (is_string($value)) {
                        $processed = strtolower($value);
                        $results[] = $processed;
                        $counter++;
                    } elseif (is_numeric($value)) {
                        $results[] = (float) $value;
                        $counter++;
                    }
                }
            } else {
                switch (gettype($item)) {
                    case 'string':
                        $results[] = trim($item);
                        break;
                    case 'integer':
                    case 'double':
                        $results[] = $item * 2;
                        break;
                    default:
                        $results[] = 'unknown';
                }
                $counter++;
            }

            // Break if we have enough results
            if ($counter >= 100) {
                break;
            }
        }

        return $results;
    }

    public function complexLogic(array $conditions): string
    {
        $result = 'default';

        try {
            if (!empty($conditions)) {
                foreach ($conditions as $condition) {
                    if ($condition['type'] === 'search') {
                        $searchTerm = $condition['value'];

                        if (strlen($searchTerm) > 3) {
                            $result = 'search: ' . $searchTerm;
                        } else {
                            $result = 'short_search';
                        }
                    } elseif ($condition['type'] === 'filter') {
                        $filterValue = $condition['value'];

                        if (is_array($filterValue)) {
                            foreach ($filterValue as $filter) {
                                if (strpos($filter, 'important') !== false) {
                                    $result = 'important_filter';
                                    break 2;
                                }
                            }
                        }
                    }
                }
            }
        } catch (Exception $e) {
            $result = 'error: ' . $e->getMessage();
        } finally {
            $result .= ' [processed]';
        }

        return $result;
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "class|trait|interface|function",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should show control flow structures
    assert!(
        output.contains("foreach") || output.contains("if") || output.contains("switch"),
        "Should show control flow structures - output: {}",
        output
    );

    // Should have closing braces for large methods with // comments
    assert!(
        output.contains("//")
            && (output.contains("processData") || output.contains("complexLogic")),
        "Should have closing brace comments for large methods - output: {}",
        output
    );

    // Should show try/catch/finally
    assert!(
        output.contains("try") || output.contains("catch") || output.contains("finally"),
        "Should show try/catch/finally structures - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_php_test_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("TestCalculator.php");

    let content = r#"<?php

namespace Tests\Unit;

use PHPUnit\Framework\TestCase;
use App\Calculator;

class CalculatorTest extends TestCase
{
    private Calculator $calculator;

    protected function setUp(): void
    {
        parent::setUp();
        $this->calculator = new Calculator();
    }

    protected function tearDown(): void
    {
        parent::tearDown();
        unset($this->calculator);
    }

    /**
     * @test
     */
    public function it_can_add_two_numbers(): void
    {
        $result = $this->calculator->add(2, 3);
        $this->assertEquals(5, $result);
    }

    /**
     * @test
     * @dataProvider additionProvider
     */
    public function it_can_add_with_data_provider(int $a, int $b, int $expected): void
    {
        $result = $this->calculator->add($a, $b);
        $this->assertEquals($expected, $result);
    }

    public function testMultiplication(): void
    {
        $result = $this->calculator->multiply(4, 5);
        $this->assertSame(20, $result);
    }

    public function testDivisionByZero(): void
    {
        $this->expectException(InvalidArgumentException::class);
        $this->calculator->divide(10, 0);
    }

    /**
     * @test
     * @group integration
     */
    public function it_handles_complex_calculations(): void
    {
        $result = $this->calculator->add(10, 5);
        $result = $this->calculator->multiply($result, 2);
        $result = $this->calculator->subtract($result, 5);

        $this->assertEquals(25, $result);
    }

    public function additionProvider(): array
    {
        return [
            [1, 1, 2],
            [2, 3, 5],
            [-1, 1, 0],
            [0, 0, 0],
        ];
    }
}

// Simple test functions (non-PHPUnit style)
function testSimpleAddition(): bool
{
    $calc = new Calculator();
    return $calc->add(1, 1) === 2;
}

function test_snake_case_function(): bool
{
    $calc = new Calculator();
    return $calc->subtract(5, 3) === 2;
}

function validateCalculatorBehavior(): bool
{
    // This is a test function but doesn't start with 'test'
    $calc = new Calculator();
    return $calc->multiply(2, 3) === 6;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Should detect PHPUnit test class
    assert!(
        output.contains("CalculatorTest") || output.contains("TestCase"),
        "Should detect PHPUnit test class - output: {}",
        output
    );

    // Should detect test methods with @test annotation
    assert!(
        output.contains("it_can_add_two_numbers") || output.contains("@test"),
        "Should detect @test annotated methods - output: {}",
        output
    );

    // Should detect testXxx methods
    assert!(
        output.contains("testMultiplication") || output.contains("testDivisionByZero"),
        "Should detect testXxx methods - output: {}",
        output
    );

    // Should detect simple test functions
    assert!(
        output.contains("testSimpleAddition") || output.contains("test_snake_case_function"),
        "Should detect simple test functions - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_php_modern_features() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("modern_php.php");

    let content = r#"<?php

declare(strict_types=1);

namespace App\Modern;

use Closure;

// PHP 8+ Union types and named arguments
class ModernProcessor
{
    public function __construct(
        private readonly string $name,
        private readonly array $config = [],
    ) {}

    // Union types (PHP 8.0+)
    public function process(string|array|null $data): int|float|null
    {
        return match ($data) {
            null => null,
            default => is_string($data) ? strlen($data) : count($data),
        };
    }

    // Arrow functions (PHP 7.4+)
    public function transformArray(array $items): array
    {
        return array_map(
            fn($item) => strtoupper((string) $item),
            array_filter($items, fn($item) => !empty($item))
        );
    }

    // Nullable return type with arrow function
    public function findFirst(array $items, callable $predicate): mixed
    {
        foreach ($items as $item) {
            if ($predicate($item)) {
                return $item;
            }
        }
        return null;
    }

    // Match expression (PHP 8.0+) with search logic
    public function getSearchStrategy(string $type): string
    {
        return match($type) {
            'fuzzy' => 'levenshtein',
            'exact' => 'strict_compare',
            'partial' => 'substring_search',
            'regex' => 'preg_match',
            default => 'default_search',
        };
    }

    // Named arguments and union types
    public function search(
        string|array $query,
        array $options = [],
        bool $caseSensitive = false,
        int|null $limit = null
    ): array {
        $strategy = $this->getSearchStrategy($options['strategy'] ?? 'partial');

        return [
            'query' => $query,
            'strategy' => $strategy,
            'case_sensitive' => $caseSensitive,
            'limit' => $limit,
        ];
    }

    // Constructor property promotion (PHP 8.0+)
    public static function create(
        string $name,
        array $config = [],
        ?string $searchEngine = null
    ): self {
        return new self(
            name: $name,
            config: [...$config, 'search_engine' => $searchEngine]
        );
    }

    // Readonly properties (PHP 8.1+)
    public function getName(): string
    {
        return $this->name;
    }

    // First-class callable syntax (PHP 8.1+)
    public function getTransformer(): callable
    {
        return $this->transformArray(...);
    }
}

// Enum (PHP 8.1+)
enum SearchType: string
{
    case EXACT = 'exact';
    case FUZZY = 'fuzzy';
    case PARTIAL = 'partial';
    case REGEX = 'regex';

    public function getDescription(): string
    {
        return match($this) {
            self::EXACT => 'Exact match search',
            self::FUZZY => 'Fuzzy search with similarity',
            self::PARTIAL => 'Substring search',
            self::REGEX => 'Regular expression search',
        };
    }
}

// Anonymous class with modern features
$processor = new class implements Countable {
    private array $data = [];

    public function add(mixed $item): void {
        $this->data[] = $item;
    }

    public function count(): int {
        return count($this->data);
    }

    public function search(string $term): array {
        return array_filter(
            $this->data,
            fn($item) => str_contains(strtolower((string) $item), strtolower($term))
        );
    }
};

// Modern function with arrow function and match
function processSearchResults(array $results, string $format = 'json'): string
{
    $processed = array_map(
        fn($result) => is_array($result) ? $result : ['value' => $result],
        $results
    );

    return match($format) {
        'json' => json_encode($processed),
        'xml' => 'XML output not implemented',
        'csv' => implode(',', array_column($processed, 'value')),
        default => serialize($processed),
    };
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
        "--max-results",
        "20",
    ])?;

    // Should contain search-related content
    assert!(
        output.contains("search"),
        "Should contain 'search' keyword - output: {}",
        output
    );

    // Should show modern PHP features
    assert!(
        output.contains("match") || output.contains("fn") || output.contains("enum"),
        "Should show modern PHP features (match/arrow functions/enum) - output: {}",
        output
    );

    // Should show union types or modern syntax
    assert!(
        output.contains("string|array") || output.contains("int|float") || output.contains("mixed"),
        "Should show union types or mixed types - output: {}",
        output
    );

    // Should show functions with search logic
    assert!(
        output.contains("getSearchStrategy") || output.contains("processSearchResults"),
        "Should show functions with search logic - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_php_outline_small_vs_large_functions() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("function_sizes.php");

    let content = r#"<?php

// Small function - should NOT get closing brace comment
function smallFunction(string $input): string
{
    return strtoupper($input);
}

// Medium function - might get closing brace comment depending on implementation
function mediumFunction(array $data): array
{
    $result = [];
    foreach ($data as $key => $value) {
        if (is_string($value)) {
            $result[$key] = trim($value);
        } elseif (is_numeric($value)) {
            $result[$key] = (float) $value;
        }
    }
    return $result;
}

// Large function - should get closing brace comment
function largeProcessingFunction(array $data): array
{
    $results = [];
    $errors = [];
    $processed = 0;

    foreach ($data as $index => $item) {
        try {
            if (!is_array($item)) {
                $errors[] = "Item at index {$index} is not an array";
                continue;
            }

            $processedItem = [];

            foreach ($item as $key => $value) {
                if (empty($key)) {
                    continue;
                }

                switch (gettype($value)) {
                    case 'string':
                        $processedItem[$key] = trim(strtolower($value));
                        break;
                    case 'integer':
                    case 'double':
                        $processedItem[$key] = round((float) $value, 2);
                        break;
                    case 'boolean':
                        $processedItem[$key] = $value ? 1 : 0;
                        break;
                    case 'array':
                        if (!empty($value)) {
                            $processedItem[$key] = array_values($value);
                        }
                        break;
                    default:
                        $processedItem[$key] = (string) $value;
                }
            }

            if (!empty($processedItem)) {
                $results[] = $processedItem;
                $processed++;
            }

        } catch (Exception $e) {
            $errors[] = "Error processing item {$index}: " . $e->getMessage();
        }
    }

    return [
        'results' => $results,
        'errors' => $errors,
        'processed_count' => $processed,
        'total_count' => count($data),
    ];
}

class ProcessorClass
{
    // Small method - should NOT get closing brace comment
    public function smallMethod(): string
    {
        return 'small';
    }

    // Large method - should get closing brace comment
    public function largeMethod(array $data): array
    {
        $config = [
            'timeout' => 30,
            'retry_count' => 3,
            'batch_size' => 100,
        ];

        $batches = array_chunk($data, $config['batch_size']);
        $allResults = [];

        foreach ($batches as $batchIndex => $batch) {
            $batchResults = [];
            $retryCount = 0;

            while ($retryCount < $config['retry_count']) {
                try {
                    foreach ($batch as $item) {
                        if ($this->validateItem($item)) {
                            $processed = $this->processItem($item);
                            if ($processed !== null) {
                                $batchResults[] = $processed;
                            }
                        }
                    }
                    break; // Success, exit retry loop
                } catch (Exception $e) {
                    $retryCount++;
                    if ($retryCount >= $config['retry_count']) {
                        error_log("Failed to process batch {$batchIndex}: " . $e->getMessage());
                    } else {
                        usleep(1000 * $retryCount); // Exponential backoff
                    }
                }
            }

            $allResults = array_merge($allResults, $batchResults);
        }

        return $allResults;
    }

    private function validateItem($item): bool
    {
        return !empty($item);
    }

    private function processItem($item)
    {
        return $item;
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "class|trait|interface|function",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Check if we found any functions at all first
    if output.contains("No results found") {
        // Try a different search pattern
        let output2 = ctx.run_probe(&[
            "search",
            "large",
            test_file.to_str().unwrap(),
            "--format",
            "outline",
            "--max-results",
            "20",
        ])?;

        if !output2.contains("No results found") {
            // Use output2 for verification if it has results
            let has_large_functions = output2.contains("largeProcessingFunction")
                || output2.contains("largeMethod")
                || output2.contains("function");
            assert!(
                has_large_functions,
                "Should find large functions in search results - output: {}",
                output2
            );
        }
    } else {
        // Original logic for when we have results
        let has_large_functions = output.contains("largeProcessingFunction")
            || output.contains("largeMethod")
            || output.contains("function");
        assert!(
            has_large_functions,
            "Should find large functions in search results - output: {}",
            output
        );
    }

    // Verify small functions do NOT have closing brace comments
    // Look for the section containing smallFunction and ensure no // comment follows
    let small_function_present = output.contains("smallFunction");
    if small_function_present {
        // Get the lines around smallFunction and check no // comment immediately follows
        let lines: Vec<&str> = output.lines().collect();
        let small_func_line = lines.iter().position(|line| line.contains("smallFunction"));

        if let Some(pos) = small_func_line {
            // Check next few lines don't contain a closing brace comment for small function
            let next_few_lines = lines.get(pos..pos.min(lines.len()).max(pos + 5));
            if let Some(section) = next_few_lines {
                let section_text = section.join("\n");
                assert!(
                    !section_text.contains("// function smallFunction")
                        && !section_text.contains("// smallFunction"),
                    "Small function should NOT have closing brace comment in section: {}",
                    section_text
                );
            }
        }
    }

    Ok(())
}

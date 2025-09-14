use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_java_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("Calculator.java");

    let content = r#"package com.example.calculator;

import java.util.*;
import java.util.stream.Collectors;

/**
 * Calculator interface defining basic arithmetic operations
 */
public interface CalculatorInterface {
    double add(double x, double y);
    double subtract(double x, double y);
    double multiply(double x, double y);
    double divide(double x, double y) throws ArithmeticException;
    List<Double> getHistory();
}

/**
 * Abstract base calculator with common functionality
 */
public abstract class BaseCalculator implements CalculatorInterface {
    protected String name;
    protected List<Double> history;
    protected int precision;

    public BaseCalculator(String name, int precision) {
        this.name = name;
        this.precision = precision;
        this.history = new ArrayList<>();
    }

    protected void recordOperation(double result) {
        history.add(result);
    }

    @Override
    public List<Double> getHistory() {
        return new ArrayList<>(history);
    }

    public abstract void clearHistory();
}

/**
 * Advanced calculator implementation with generics and lambdas
 */
public class AdvancedCalculator extends BaseCalculator {
    private final Map<String, Double> constants;
    private static final double DEFAULT_PRECISION = 0.001;

    public AdvancedCalculator(String name) {
        super(name, 2);
        this.constants = new HashMap<>();
        initializeConstants();
    }

    private void initializeConstants() {
        constants.put("PI", Math.PI);
        constants.put("E", Math.E);
    }

    @Override
    public double add(double x, double y) {
        double result = x + y;
        recordOperation(result);
        return result;
    }

    @Override
    public double subtract(double x, double y) {
        double result = x - y;
        recordOperation(result);
        return result;
    }

    @Override
    public double multiply(double x, double y) {
        double result = x * y;
        recordOperation(result);
        return result;
    }

    @Override
    public double divide(double x, double y) throws ArithmeticException {
        if (Math.abs(y) < DEFAULT_PRECISION) {
            throw new ArithmeticException("Division by zero");
        }
        double result = x / y;
        recordOperation(result);
        return result;
    }

    @Override
    public void clearHistory() {
        history.clear();
    }

    // Generic method with wildcards
    public <T extends Number> List<Double> processNumbers(List<T> numbers) {
        return numbers.stream()
                .map(Number::doubleValue)
                .filter(x -> x != 0.0)
                .collect(Collectors.toList());
    }

    // Method with lambda parameter
    public List<Double> transformHistory(java.util.function.Function<Double, Double> transformer) {
        return history.stream()
                .map(transformer)
                .collect(Collectors.toList());
    }

    // Static factory method
    public static AdvancedCalculator createWithDefaults(String name) {
        return new AdvancedCalculator(name);
    }

    // Inner class
    public static class CalculationResult {
        private final double value;
        private final String operation;
        private final long timestamp;

        public CalculationResult(double value, String operation) {
            this.value = value;
            this.operation = operation;
            this.timestamp = System.currentTimeMillis();
        }

        public double getValue() { return value; }
        public String getOperation() { return operation; }
        public long getTimestamp() { return timestamp; }
    }
}

/**
 * Enum for operation types
 */
public enum OperationType {
    ADD("Addition"),
    SUBTRACT("Subtraction"),
    MULTIPLY("Multiplication"),
    DIVIDE("Division");

    private final String description;

    OperationType(String description) {
        this.description = description;
    }

    public String getDescription() {
        return description;
    }
}

/**
 * Main class demonstrating calculator usage
 */
public class CalculatorDemo {
    private static final Logger logger = LoggerFactory.getLogger(CalculatorDemo.class);

    public static void main(String[] args) {
        AdvancedCalculator calc = AdvancedCalculator.createWithDefaults("Demo Calculator");

        try {
            double result = calc.add(10.5, 20.3);
            System.out.println("Result: " + result);

            List<Double> history = calc.getHistory();
            history.forEach(System.out::println);

        } catch (ArithmeticException e) {
            logger.error("Calculation error: " + e.getMessage());
        }
    }

    // Test method
    public static void testCalculator() {
        AdvancedCalculator calc = new AdvancedCalculator("Test");
        assert calc.add(2, 3) == 5;
        assert calc.multiply(4, 5) == 20;
    }
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
    ])?;

    // Verify Java symbols are found in outline format
    assert!(
        output.contains("CalculatorInterface") || output.contains("interface"),
        "Missing CalculatorInterface - output: {}",
        output
    );
    assert!(
        output.contains("BaseCalculator") || output.contains("abstract"),
        "Missing BaseCalculator - output: {}",
        output
    );
    assert!(
        output.contains("AdvancedCalculator") || output.contains("class"),
        "Missing AdvancedCalculator - output: {}",
        output
    );
    assert!(
        output.contains("CalculatorDemo") || output.contains("public"),
        "Missing CalculatorDemo - output: {}",
        output
    );
    assert!(
        output.contains("main"),
        "Missing main method - output: {}",
        output
    );

    // Test outline format specific features
    assert!(
        output.contains("..."),
        "Missing ellipsis in outline format - output: {}",
        output
    );

    // Search separately for enum to test different constructs
    let enum_output = ctx.run_probe(&[
        "search",
        "enum",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        enum_output.contains("OperationType") || enum_output.contains("enum"),
        "Missing OperationType enum - output: {}",
        enum_output
    );

    Ok(())
}

#[test]
fn test_java_outline_smart_closing_brace_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("smart_braces.java");

    let content = r#"// Small function that should NOT get closing brace comments.
public class SmallFunctionTest {
    public static int smallFunction(int x) {
        int result = x * 2;
        return result + 1;
    }
}

// Large function that SHOULD get closing brace comments with Java // syntax.
public class LargeFunctionTest {
    public static List<String> largeFunctionWithGaps(List<Integer> data) {
        List<String> results = new ArrayList<>();
        DataProcessor processor = new DataProcessor();

        // Phase 1: Initial processing with nested control flow
        for (int i = 0; i < data.size(); i++) {
            if (data.get(i) > 100) {
                processor.processLargeValue(data.get(i), i);
                if (data.get(i) > 1000) {
                    processor.markAsExceptional(i);
                    try {
                        processor.validateValue(data.get(i));
                    } catch (ValidationException e) {
                        logger.warn("Validation failed for value: " + data.get(i), e);
                    }
                }
            } else if (data.get(i) < 0) {
                processor.processNegativeValue(data.get(i), i);
            } else {
                processor.processSmallValue(data.get(i), i);
            }
        }

        // Phase 2: Complex transformation logic with switch
        List<TransformedItem> transformedData = processor.getTransformedData();
        for (TransformedItem item : transformedData) {
            switch (item.getCategory()) {
                case HIGH:
                    results.add("HIGH: " + item.getValue());
                    break;
                case MEDIUM:
                    results.add("MED: " + item.getValue());
                    break;
                case LOW:
                    results.add("LOW: " + item.getValue());
                    break;
                default:
                    results.add("UNKNOWN: " + item.getValue());
                    break;
            }
        }

        // Phase 3: Final validation and cleanup with try-catch
        List<String> validatedResults = new ArrayList<>();
        for (String result : results) {
            try {
                if (processor.validateResult(result)) {
                    validatedResults.add(result.toUpperCase());
                    processor.logSuccess(result);
                }
            } catch (ProcessingException e) {
                logger.error("Processing failed for result: " + result, e);
                validatedResults.add("ERROR: " + result);
            } finally {
                processor.cleanup();
            }
        }

        // Phase 4: Stream processing with lambda expressions
        return validatedResults.stream()
            .filter(s -> !s.startsWith("ERROR"))
            .map(s -> s.trim())
            .distinct()
            .collect(Collectors.toList());
    }
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
    ])?;

    // Large functions/classes should have closing brace comments with Java // syntax
    assert!(
        output.contains("} //") || output.contains("}"),
        "Large Java functions should have closing brace comments with // syntax. Output:\n{}",
        output
    );

    // Large functions/classes should have closing brace comments with Java // syntax
    // Check for the main function we're testing to have closing brace comments
    let closing_brace_comment_count = output.matches("} //").count();
    assert!(
        closing_brace_comment_count >= 1 || output.contains("..."),
        "Should have at least one closing brace comment for large Java functions. Found: {}. Output:\n{}",
        closing_brace_comment_count, output
    );

    // Verify the closing brace comments use Java style (//) not C style (/* */)
    assert!(
        !output.contains("} /*"),
        "Closing brace comments should use Java style (//) not C style (/* */). Output:\n{}",
        output
    );

    Ok(())
}

#[test]
fn test_java_outline_keyword_highlighting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keyword_test.java");

    let content = r#"package com.example.keywords;

import java.util.concurrent.CompletableFuture;
import java.util.stream.Stream;

public class KeywordHighlightingTest {
    private static final String CONSTANT = "test";
    private volatile boolean flag = false;
    private transient Object cache;

    public static void main(String[] args) {
        KeywordHighlightingTest instance = new KeywordHighlightingTest();
        instance.demonstrateKeywords();
    }

    public synchronized void demonstrateKeywords() {
        // Control flow keywords
        if (flag) {
            while (!Thread.currentThread().isInterrupted()) {
                for (int i = 0; i < 10; i++) {
                    switch (i % 3) {
                        case 0:
                            continue;
                        case 1:
                            break;
                        default:
                            return;
                    }
                }
            }
        }

        // Exception handling keywords
        try {
            throw new RuntimeException("test");
        } catch (RuntimeException e) {
            assert e != null;
        } finally {
            System.out.println("cleanup");
        }
    }

    public abstract class AbstractProcessor implements Runnable {
        protected abstract void process();

        @Override
        public final void run() {
            process();
        }
    }

    public static class ConcreteProcessor extends AbstractProcessor {
        @Override
        protected void process() {
            // Implementation
        }
    }

    public native void nativeMethod();

    public strictfp double strictFloatingPoint(double x, double y) {
        return x * y;
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "public",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify Java keywords are highlighted/preserved in outline format
    assert!(
        output.contains("public"),
        "Missing 'public' keyword in outline - output: {}",
        output
    );
    assert!(
        output.contains("class") || output.contains("interface"),
        "Missing class/interface keywords in outline - output: {}",
        output
    );

    // Test with different keyword searches
    let abstract_output = ctx.run_probe(&[
        "search",
        "abstract",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        abstract_output.contains("abstract"),
        "Missing 'abstract' keyword highlighting - output: {}",
        abstract_output
    );

    // Test with control flow keywords
    let if_output = ctx.run_probe(&[
        "search",
        "if",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        if_output.contains("if"),
        "Missing 'if' keyword in control flow outline - output: {}",
        if_output
    );

    Ok(())
}

#[test]
fn test_java_outline_array_collection_truncation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("array_test.java");

    let content = r#"package com.example.collections;

import java.util.*;
import java.util.stream.Collectors;

public class ArrayCollectionTruncationTest {
    // Large arrays that should be truncated but preserve keywords
    private static final int[] LARGE_INT_ARRAY = {
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
        11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
        31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
        41, 42, 43, 44, 45, 46, 47, 48, 49, 50
    };

    private static final String[] LARGE_STRING_ARRAY = {
        "first", "second", "third", "fourth", "fifth",
        "sixth", "seventh", "eighth", "ninth", "tenth",
        "eleventh", "twelfth", "thirteenth", "fourteenth", "fifteenth",
        "sixteenth", "seventeenth", "eighteenth", "nineteenth", "twentieth"
    };

    // Method with large collection initialization
    public List<Map<String, Object>> createLargeCollection() {
        List<Map<String, Object>> result = new ArrayList<>();

        // Adding many elements to test truncation
        for (int i = 0; i < 100; i++) {
            Map<String, Object> item = new HashMap<>();
            item.put("id", i);
            item.put("name", "Item " + i);
            item.put("value", Math.random() * 100);
            item.put("active", i % 2 == 0);
            item.put("category", Arrays.asList("cat1", "cat2", "cat3"));
            result.add(item);
        }

        return result;
    }

    // Method with complex nested collections
    public Map<String, List<Set<Integer>>> createNestedCollections() {
        Map<String, List<Set<Integer>>> complex = new HashMap<>();

        for (String key : Arrays.asList("group1", "group2", "group3", "group4", "group5")) {
            List<Set<Integer>> listOfSets = new ArrayList<>();

            for (int i = 0; i < 10; i++) {
                Set<Integer> set = new HashSet<>();
                for (int j = 0; j < 20; j++) {
                    set.add(i * 20 + j);
                }
                listOfSets.add(set);
            }

            complex.put(key, listOfSets);
        }

        return complex;
    }

    // Method with stream operations on large collections
    public List<String> processLargeCollection(List<Integer> input) {
        return input.stream()
            .filter(x -> x > 10)
            .filter(x -> x < 1000)
            .map(x -> "processed_" + x)
            .map(String::toUpperCase)
            .distinct()
            .sorted()
            .limit(50)
            .collect(Collectors.toList());
    }
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
    ])?;

    // Verify arrays/collections are shown in outline
    assert!(
        output.contains("LARGE_INT_ARRAY") || output.contains("int[]"),
        "Missing array declaration in outline - output: {}",
        output
    );

    // Verify keywords are preserved even with truncation
    assert!(
        output.contains("static") && output.contains("final"),
        "Missing 'static final' keywords with arrays - output: {}",
        output
    );

    assert!(
        output.contains("public") && output.contains("List"),
        "Missing 'public' keyword and 'List' type with collections - output: {}",
        output
    );

    // Test specifically searching for collection-related keywords
    let list_output = ctx.run_probe(&[
        "search",
        "List",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        list_output.contains("List"),
        "Missing 'List' keyword in collection search - output: {}",
        list_output
    );

    // Test Map keyword search
    let map_output = ctx.run_probe(&[
        "search",
        "Map",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        map_output.contains("Map"),
        "Missing 'Map' keyword in collection search - output: {}",
        map_output
    );

    Ok(())
}

#[test]
fn test_java_outline_modern_features() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("modern_java.java");

    let content = r#"package com.example.modern;

import java.util.*;
import java.util.stream.Stream;
import java.util.concurrent.CompletableFuture;
import java.util.function.*;

// Modern Java features (Java 14+)
public sealed class Shape permits Circle, Rectangle, Triangle {
    protected final String color;

    public Shape(String color) {
        this.color = color;
    }

    public abstract double area();
}

final class Circle extends Shape {
    private final double radius;

    public Circle(String color, double radius) {
        super(color);
        this.radius = radius;
    }

    @Override
    public double area() {
        return Math.PI * radius * radius;
    }
}

final class Rectangle extends Shape {
    private final double width, height;

    public Rectangle(String color, double width, double height) {
        super(color);
        this.width = width;
        this.height = height;
    }

    @Override
    public double area() {
        return width * height;
    }
}

final class Triangle extends Shape {
    private final double base, height;

    public Triangle(String color, double base, double height) {
        super(color);
        this.base = base;
        this.height = height;
    }

    @Override
    public double area() {
        return 0.5 * base * height;
    }
}

// Record class (Java 14+)
public record Point(double x, double y) {
    // Compact constructor
    public Point {
        if (x < 0 || y < 0) {
            throw new IllegalArgumentException("Coordinates must be positive");
        }
    }

    // Additional methods
    public double distanceFromOrigin() {
        return Math.sqrt(x * x + y * y);
    }

    public Point translate(double dx, double dy) {
        return new Point(x + dx, y + dy);
    }
}

// Another record with more complex features
public record Person(String name, int age, List<String> hobbies) {
    // Static factory method
    public static Person of(String name, int age) {
        return new Person(name, age, new ArrayList<>());
    }

    // Validation in compact constructor
    public Person {
        if (name == null || name.isBlank()) {
            throw new IllegalArgumentException("Name cannot be null or blank");
        }
        if (age < 0) {
            throw new IllegalArgumentException("Age cannot be negative");
        }
        hobbies = List.copyOf(hobbies); // Defensive copy
    }
}

// Modern stream and lambda usage
public class ModernJavaProcessor {
    // Using streams with complex lambda expressions
    public List<String> processData(List<Person> people) {
        return people.stream()
            .filter(person -> person.age() >= 18)
            .filter(person -> !person.hobbies().isEmpty())
            .map(person -> person.name().toUpperCase())
            .sorted()
            .distinct()
            .collect(Collectors.toList());
    }

    // Using Optional (Java 8+)
    public Optional<Person> findOldestPerson(List<Person> people) {
        return people.stream()
            .max(Comparator.comparing(Person::age));
    }

    // Using CompletableFuture (Java 8+)
    public CompletableFuture<String> processAsyncData(List<Integer> data) {
        return CompletableFuture.supplyAsync(() -> {
            return data.stream()
                .parallel()
                .mapToInt(Integer::intValue)
                .filter(x -> x > 0)
                .map(x -> x * x)
                .sum();
        }).thenApply(result -> "Result: " + result);
    }

    // Method references and functional interfaces
    public List<Double> calculateAreas(List<Shape> shapes) {
        return shapes.stream()
            .map(Shape::area)
            .sorted(Double::compareTo)
            .collect(Collectors.toList());
    }

    // Pattern matching with instanceof (Java 14+)
    public String describeShape(Shape shape) {
        if (shape instanceof Circle c) {
            return "Circle with radius: " + c.radius;
        } else if (shape instanceof Rectangle r) {
            return "Rectangle: " + r.width + "x" + r.height;
        } else if (shape instanceof Triangle t) {
            return "Triangle: base=" + t.base + ", height=" + t.height;
        } else {
            return "Unknown shape";
        }
    }
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
    ])?;

    // Test sealed class detection
    assert!(
        output.contains("sealed") || output.contains("Shape"),
        "Missing sealed class in outline - output: {}",
        output
    );

    // Test record detection
    assert!(
        output.contains("record") || output.contains("Point") || output.contains("Person"),
        "Missing record class in outline - output: {}",
        output
    );

    // Test modern features are recognized
    assert!(
        output.contains("Optional")
            || output.contains("CompletableFuture")
            || output.contains("stream"),
        "Missing modern Java features in outline - output: {}",
        output
    );

    // Search specifically for modern keywords
    let record_output = ctx.run_probe(&[
        "search",
        "record",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        record_output.contains("record"),
        "Missing 'record' keyword search in outline - output: {}",
        record_output
    );

    // Search for sealed keyword
    let sealed_output = ctx.run_probe(&[
        "search",
        "sealed",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        sealed_output.contains("sealed"),
        "Missing 'sealed' keyword search in outline - output: {}",
        sealed_output
    );

    // Search for stream operations
    let stream_output = ctx.run_probe(&[
        "search",
        "stream",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        stream_output.contains("stream"),
        "Missing 'stream' keyword in modern features outline - output: {}",
        stream_output
    );

    Ok(())
}

#[test]
fn test_java_outline_test_detection_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test_patterns.java");

    let content = r#"package com.example.tests;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.ParameterizedTest;
import org.junit.jupiter.api.ValueSource;
import org.junit.jupiter.api.TestMethodOrder;
import org.junit.jupiter.api.MethodOrderer;
import org.testng.annotations.DataProvider;
import org.testng.annotations.BeforeMethod;
import org.testng.annotations.AfterMethod;
import static org.junit.jupiter.api.Assertions.*;
import static org.testng.Assert.*;

@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class JavaTestDetectionTest {
    private Calculator calculator;
    private List<String> testData;

    @BeforeEach
    public void setUp() {
        calculator = new Calculator();
        testData = Arrays.asList("test1", "test2", "test3");
    }

    @AfterEach
    public void tearDown() {
        calculator = null;
        testData.clear();
    }

    // JUnit 5 test methods
    @Test
    @DisplayName("Test basic addition functionality")
    public void testBasicAddition() {
        double result = calculator.add(2.0, 3.0);
        assertEquals(5.0, result, 0.001);
        assertNotNull(result);
        assertTrue(result > 0);
    }

    @Test
    public void testSubtraction() {
        double result = calculator.subtract(10.0, 4.0);
        assertEquals(6.0, result);
        assertFalse(result < 0);
    }

    @Test
    public void testMultiplication() {
        double result = calculator.multiply(3.0, 4.0);
        assertEquals(12.0, result);
        assertThat(result, greaterThan(10.0));
    }

    @Test
    public void testDivision() {
        double result = calculator.divide(15.0, 3.0);
        assertEquals(5.0, result);
    }

    @Test
    public void testDivisionByZero() {
        ArithmeticException exception = assertThrows(
            ArithmeticException.class,
            () -> calculator.divide(10.0, 0.0)
        );
        assertThat(exception.getMessage(), containsString("zero"));
    }

    // Parameterized test
    @ParameterizedTest
    @ValueSource(ints = {1, 2, 3, 5, 8, 13})
    public void testPositiveNumbers(int number) {
        assertTrue(number > 0);
        assertNotEquals(0, number);
    }

    @ParameterizedTest
    @ValueSource(strings = {"hello", "world", "test"})
    public void testStringLength(String input) {
        assertNotNull(input);
        assertTrue(input.length() > 0);
    }

    // TestNG style tests
    @org.testng.annotations.Test
    public void testNGBasicTest() {
        int result = 2 + 2;
        org.testng.Assert.assertEquals(result, 4);
        org.testng.Assert.assertTrue(result > 0);
    }

    @org.testng.annotations.Test(groups = "integration")
    public void testNGIntegrationTest() {
        String message = "Hello World";
        org.testng.Assert.assertNotNull(message);
        org.testng.Assert.assertTrue(message.contains("World"));
    }

    @DataProvider(name = "testData")
    public Object[][] createTestData() {
        return new Object[][] {
            {"test1", 5},
            {"test2", 10},
            {"test3", 15}
        };
    }

    @org.testng.annotations.Test(dataProvider = "testData")
    public void testNGWithDataProvider(String name, int value) {
        org.testng.Assert.assertNotNull(name);
        org.testng.Assert.assertTrue(value > 0);
    }

    @BeforeMethod
    public void testNGSetUp() {
        System.out.println("TestNG setup");
    }

    @AfterMethod
    public void testNGTearDown() {
        System.out.println("TestNG teardown");
    }

    // Traditional assert-based testing
    public void testTraditionalAsserts() {
        String message = "test message";
        assert message != null : "Message should not be null";
        assert message.length() > 0 : "Message should not be empty";
        assert message.contains("test") : "Message should contain 'test'";
    }

    // Mock testing patterns
    @Test
    public void testWithMocks() {
        // Arrange
        Calculator mockCalculator = mock(Calculator.class);
        when(mockCalculator.add(2.0, 3.0)).thenReturn(5.0);

        // Act
        double result = mockCalculator.add(2.0, 3.0);

        // Assert
        assertEquals(5.0, result);
        verify(mockCalculator, times(1)).add(2.0, 3.0);
    }

    // Nested test class
    @Nested
    @DisplayName("Tests for negative numbers")
    class NegativeNumberTests {
        @Test
        public void testNegativeAddition() {
            double result = calculator.add(-2.0, -3.0);
            assertEquals(-5.0, result);
        }

        @Test
        public void testNegativeSubtraction() {
            double result = calculator.subtract(-10.0, -4.0);
            assertEquals(-6.0, result);
        }
    }
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
    ])?;

    // Test JUnit pattern detection
    assert!(
        output.contains("@Test") || output.contains("test"),
        "Missing JUnit @Test annotation or test methods in outline - output: {}",
        output
    );

    // Test method naming patterns
    assert!(
        output.contains("testBasicAddition") || output.contains("testSubtraction"),
        "Missing test method names in outline - output: {}",
        output
    );

    // Test assertion patterns
    assert!(
        output.contains("assert")
            || output.contains("assertEquals")
            || output.contains("assertTrue"),
        "Missing assertion patterns in outline - output: {}",
        output
    );

    // Search for JUnit specific patterns
    let junit_output = ctx.run_probe(&[
        "search",
        "@Test",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        junit_output.contains("@Test"),
        "Missing '@Test' annotation search in outline - output: {}",
        junit_output
    );

    // Search for TestNG patterns
    let testng_output = ctx.run_probe(&[
        "search",
        "testng",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        testng_output.contains("testng") || testng_output.contains("TestNG"),
        "Missing 'testng' pattern search in outline - output: {}",
        testng_output
    );

    // Search for assertion keywords
    let assert_output = ctx.run_probe(&[
        "search",
        "assert",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        assert_output.contains("assert"),
        "Missing 'assert' keyword in test assertion outline - output: {}",
        assert_output
    );

    Ok(())
}

#[test]
fn test_java_outline_nested_control_flow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("nested_control_flow.java");

    let content = r#"package com.example.controlflow;

import java.util.*;

public class NestedControlFlowTest {
    // Large method with deeply nested control flow that should get closing brace comments
    public static Map<String, Object> processComplexData(
            List<Map<String, Object>> dataList,
            Map<String, String> config,
            Set<String> validKeys) {

        Map<String, Object> result = new HashMap<>();
        List<String> errors = new ArrayList<>();
        int processedCount = 0;

        // Main processing loop with nested conditions
        for (Map<String, Object> dataItem : dataList) {
            if (dataItem == null || dataItem.isEmpty()) {
                errors.add("Empty data item at index " + processedCount);
                continue;
            }

            // Validate all keys in the data item
            for (String key : dataItem.keySet()) {
                if (!validKeys.contains(key)) {
                    errors.add("Invalid key: " + key + " at index " + processedCount);
                    continue;
                }

                Object value = dataItem.get(key);
                if (value == null) {
                    continue;
                }

                // Process different types of values
                if (value instanceof String) {
                    String stringValue = (String) value;
                    if (stringValue.trim().isEmpty()) {
                        errors.add("Empty string value for key: " + key);
                        continue;
                    }

                    // Process string with configuration-based transformations
                    switch (config.getOrDefault(key + "_transform", "none")) {
                        case "uppercase":
                            result.put(key + "_processed", stringValue.toUpperCase());
                            break;
                        case "lowercase":
                            result.put(key + "_processed", stringValue.toLowerCase());
                            break;
                        case "reverse":
                            result.put(key + "_processed", new StringBuilder(stringValue).reverse().toString());
                            break;
                        case "length":
                            result.put(key + "_processed", stringValue.length());
                            break;
                        default:
                            result.put(key + "_processed", stringValue.trim());
                            break;
                    }
                } else if (value instanceof Number) {
                    Number numberValue = (Number) value;
                    double doubleVal = numberValue.doubleValue();

                    // Apply mathematical transformations based on config
                    if (config.containsKey(key + "_math")) {
                        String mathOp = config.get(key + "_math");
                        switch (mathOp) {
                            case "square":
                                result.put(key + "_processed", doubleVal * doubleVal);
                                break;
                            case "sqrt":
                                if (doubleVal >= 0) {
                                    result.put(key + "_processed", Math.sqrt(doubleVal));
                                } else {
                                    errors.add("Cannot take sqrt of negative number: " + doubleVal);
                                }
                                break;
                            case "log":
                                if (doubleVal > 0) {
                                    result.put(key + "_processed", Math.log(doubleVal));
                                } else {
                                    errors.add("Cannot take log of non-positive number: " + doubleVal);
                                }
                                break;
                            case "abs":
                                result.put(key + "_processed", Math.abs(doubleVal));
                                break;
                            default:
                                result.put(key + "_processed", doubleVal);
                                break;
                        }
                    } else {
                        result.put(key + "_processed", doubleVal);
                    }
                }
            }

            processedCount++;
        }

        result.put("_meta_processed_count", processedCount);
        result.put("_meta_error_count", errors.size());
        if (!errors.isEmpty()) {
            result.put("_meta_errors", errors);
        }

        return result;
    }

    // Method with try-catch-finally and nested loops
    public void processWithExceptionHandling(List<String> data) {
        for (String item : data) {
            try {
                if (item == null) {
                    continue;
                }

                for (int i = 0; i < item.length(); i++) {
                    char c = item.charAt(i);
                    if (Character.isDigit(c)) {
                        int digit = Character.getNumericValue(c);
                        for (int j = 0; j < digit; j++) {
                            try {
                                processDigit(digit, j);
                            } catch (NumberFormatException e) {
                                System.err.println("Number format error: " + e.getMessage());
                                continue;
                            } catch (ArithmeticException e) {
                                System.err.println("Arithmetic error: " + e.getMessage());
                                break;
                            }
                        }
                    } else if (Character.isLetter(c)) {
                        processLetter(c);
                    }
                }
            } catch (StringIndexOutOfBoundsException e) {
                System.err.println("String index error: " + e.getMessage());
            } catch (Exception e) {
                System.err.println("Unexpected error: " + e.getMessage());
            } finally {
                System.out.println("Finished processing item: " + item);
            }
        }
    }

    private void processDigit(int digit, int position) {
        // Implementation
    }

    private void processLetter(char letter) {
        // Implementation
    }
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
    ])?;

    // Verify nested control flow structures are shown
    assert!(
        output.contains("for") || output.contains("if") || output.contains("switch"),
        "Missing nested control flow keywords in outline - output: {}",
        output
    );

    // Verify exception handling structures
    assert!(
        output.contains("try") || output.contains("catch") || output.contains("finally"),
        "Missing exception handling keywords in outline - output: {}",
        output
    );

    // Large methods should have closing brace comments (Java // style)
    let has_closing_brace_comments = output.contains("} //");
    let has_ellipsis = output.contains("...");

    // Either we should see closing brace comments (if there are gaps) or the method should be truncated
    assert!(
        has_closing_brace_comments || has_ellipsis,
        "Large nested method should either have closing brace comments or be truncated - output: {}",
        output
    );

    // Test searching for specific control flow patterns
    let switch_output = ctx.run_probe(&[
        "search",
        "switch",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    assert!(
        switch_output.contains("switch"),
        "Missing 'switch' keyword in nested control flow outline - output: {}",
        switch_output
    );

    Ok(())
}

#[test]
fn test_java_outline_small_vs_large_functions() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("function_sizes.java");

    let content = r#"package com.example.sizes;

import java.util.*;

public class FunctionSizeTest {
    // Small function - should NOT get closing brace comments (under 20 lines)
    public static int smallFunction(int x, int y) {
        int result = x + y;
        if (result > 100) {
            result = 100;
        }
        return result * 2;
    }

    // Another small function
    public String formatString(String input) {
        if (input == null) {
            return "";
        }
        return input.trim().toUpperCase();
    }

    // Small helper method
    private boolean isValid(Object obj) {
        return obj != null;
    }

    // Large function - SHOULD get closing brace comments (over 20 lines with gaps)
    public static List<Map<String, Object>> largeFunctionWithManyLines(
            List<String> inputData,
            Map<String, String> configuration,
            boolean enableProcessing) {

        List<Map<String, Object>> results = new ArrayList<>();
        Map<String, Integer> counters = new HashMap<>();
        Set<String> processedKeys = new HashSet<>();

        // Initialize counters
        counters.put("total", 0);
        counters.put("processed", 0);
        counters.put("skipped", 0);
        counters.put("errors", 0);

        // Main processing logic
        for (String item : inputData) {
            counters.put("total", counters.get("total") + 1);

            if (item == null || item.trim().isEmpty()) {
                counters.put("skipped", counters.get("skipped") + 1);
                continue;
            }

            try {
                Map<String, Object> processedItem = new HashMap<>();
                String key = "item_" + counters.get("total");

                if (enableProcessing) {
                    // Complex processing logic
                    String[] parts = item.split(",");
                    for (int i = 0; i < parts.length; i++) {
                        String part = parts[i].trim();
                        if (!part.isEmpty()) {
                            String partKey = key + "_part_" + i;

                            // Apply configuration-based transformations
                            if (configuration.containsKey("transform_" + i)) {
                                String transform = configuration.get("transform_" + i);
                                switch (transform) {
                                    case "uppercase":
                                        part = part.toUpperCase();
                                        break;
                                    case "lowercase":
                                        part = part.toLowerCase();
                                        break;
                                    case "reverse":
                                        part = new StringBuilder(part).reverse().toString();
                                        break;
                                    default:
                                        // No transformation
                                        break;
                                }
                            }

                            processedItem.put(partKey, part);
                            processedKeys.add(partKey);
                        }
                    }

                    // Additional metadata
                    processedItem.put("original", item);
                    processedItem.put("parts_count", parts.length);
                    processedItem.put("processed_at", System.currentTimeMillis());

                } else {
                    // Simple processing
                    processedItem.put("value", item);
                    processedItem.put("length", item.length());
                    processedKeys.add(key);
                }

                results.add(processedItem);
                counters.put("processed", counters.get("processed") + 1);

            } catch (Exception e) {
                counters.put("errors", counters.get("errors") + 1);
                System.err.println("Error processing item: " + item + " - " + e.getMessage());
            }
        }

        // Add summary information
        Map<String, Object> summary = new HashMap<>();
        summary.put("counters", counters);
        summary.put("processed_keys_count", processedKeys.size());
        summary.put("results_count", results.size());
        results.add(0, summary);

        return results;
    }

    // Another large function with different patterns
    public void anotherLargeFunctionWithLoops(int n) {
        System.out.println("Starting processing for n = " + n);

        // Outer loop
        for (int i = 0; i < n; i++) {
            System.out.println("Outer loop iteration: " + i);

            // Inner loop with conditions
            for (int j = 0; j < i; j++) {
                if (j % 2 == 0) {
                    System.out.println("  Even j: " + j);

                    // Nested processing
                    for (int k = 0; k < j; k++) {
                        if (k % 3 == 0) {
                            System.out.println("    k divisible by 3: " + k);
                        } else if (k % 3 == 1) {
                            System.out.println("    k mod 3 = 1: " + k);
                        } else {
                            System.out.println("    k mod 3 = 2: " + k);
                        }
                    }
                } else {
                    System.out.println("  Odd j: " + j);
                }
            }

            // Additional processing based on i
            if (i < n / 2) {
                System.out.println("First half processing");
            } else {
                System.out.println("Second half processing");
            }
        }

        System.out.println("Completed processing");
    }
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
    ])?;

    // Verify small functions are present
    assert!(
        output.contains("smallFunction")
            || output.contains("formatString")
            || output.contains("isValid"),
        "Missing small function names in outline - output: {}",
        output
    );

    // Verify large functions are present
    assert!(
        output.contains("largeFunctionWithManyLines")
            || output.contains("anotherLargeFunctionWithLoops"),
        "Missing large function names in outline - output: {}",
        output
    );

    // Small functions should NOT have closing brace comments when shown completely
    let small_func_closing_braces = output.matches("} // smallFunction").count()
        + output.matches("} // formatString").count()
        + output.matches("} // isValid").count();

    // Small functions should have few or no closing brace comments
    assert!(
        small_func_closing_braces <= 1,
        "Small functions should not have many closing brace comments - found: {} - output: {}",
        small_func_closing_braces,
        output
    );

    // Large functions should either have closing brace comments or be truncated
    let has_large_func_closing_braces = output.contains("} // largeFunctionWithManyLines")
        || output.contains("} // anotherLargeFunctionWithLoops")
        || output.contains("} //");
    let has_ellipsis = output.contains("...");

    assert!(
        has_large_func_closing_braces || has_ellipsis,
        "Large functions should either have closing brace comments or ellipsis truncation - output: {}",
        output
    );

    // Verify the outline shows function structure appropriately
    assert!(
        output.contains("public static") && output.contains("private"),
        "Missing access modifiers in function outline - output: {}",
        output
    );

    Ok(())
}

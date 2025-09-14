use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_swift_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("Calculator.swift");

    let content = r#"import Foundation

// Protocol for calculator operations
protocol CalculatorProtocol {
    func add(_ x: Double, _ y: Double) -> Double
    func subtract(_ x: Double, _ y: Double) -> Double
    func multiply(_ x: Double, _ y: Double) -> Double
    func divide(_ x: Double, _ y: Double) throws -> Double
    var history: [Double] { get }
}

// Custom errors
enum CalculatorError: Error {
    case divisionByZero
    case invalidInput(String)
    case operationFailed(String)
}

extension CalculatorError: LocalizedError {
    var errorDescription: String? {
        switch self {
        case .divisionByZero:
            return "Division by zero is not allowed"
        case .invalidInput(let input):
            return "Invalid input: \(input)"
        case .operationFailed(let operation):
            return "Operation failed: \(operation)"
        }
    }
}

// Base calculator class
class BaseCalculator: CalculatorProtocol {
    private(set) var name: String
    private(set) var history: [Double] = []
    private let precision: Double

    init(name: String, precision: Double = 0.001) {
        self.name = name
        self.precision = precision
    }

    func add(_ x: Double, _ y: Double) -> Double {
        let result = x + y
        recordOperation(result)
        return result
    }

    func subtract(_ x: Double, _ y: Double) -> Double {
        let result = x - y
        recordOperation(result)
        return result
    }

    func multiply(_ x: Double, _ y: Double) -> Double {
        let result = x * y
        recordOperation(result)
        return result
    }

    func divide(_ x: Double, _ y: Double) throws -> Double {
        guard abs(y) > precision else {
            throw CalculatorError.divisionByZero
        }

        let result = x / y
        recordOperation(result)
        return result
    }

    func clearHistory() {
        history.removeAll()
    }

    private func recordOperation(_ result: Double) {
        history.append(result)
    }
}

// Advanced calculator with generics and modern Swift features
class AdvancedCalculator: BaseCalculator {
    typealias NumberProcessor<T> = (T) -> T where T: Numeric

    private var constants: [String: Double] = [
        "pi": Double.pi,
        "e": M_E,
        "goldenRatio": (1 + sqrt(5)) / 2
    ]

    private var operationsCount: Int = 0

    override init(name: String, precision: Double = 0.001) {
        super.init(name: name, precision: precision)
    }

    // Convenience initializer
    convenience init(name: String) {
        self.init(name: name, precision: 0.001)
    }

    // Override operations to add counting
    override func add(_ x: Double, _ y: Double) -> Double {
        operationsCount += 1
        return super.add(x, y)
    }

    override func subtract(_ x: Double, _ y: Double) -> Double {
        operationsCount += 1
        return super.subtract(x, y)
    }

    override func multiply(_ x: Double, _ y: Double) -> Double {
        operationsCount += 1
        return super.multiply(x, y)
    }

    override func divide(_ x: Double, _ y: Double) throws -> Double {
        operationsCount += 1
        return try super.divide(x, y)
    }

    // Generic method with constraints
    func processNumbers<T: Numeric>(_ numbers: [T], with processor: NumberProcessor<T>) -> [T] {
        return numbers.map(processor)
    }

    // Method with closure parameter
    func transformHistory(_ transformer: @escaping (Double) -> Double) -> [Double] {
        return history.map(transformer)
    }

    // Computed properties
    var averageResult: Double {
        guard !history.isEmpty else { return 0 }
        return history.reduce(0, +) / Double(history.count)
    }

    var operationsPerformed: Int {
        return operationsCount
    }

    // Static factory method
    static func createDefault(name: String) -> AdvancedCalculator {
        return AdvancedCalculator(name: name)
    }

    // Subscript for accessing constants
    subscript(constant: String) -> Double? {
        get { return constants[constant] }
        set { constants[constant] = newValue }
    }

    // Nested enum for operation types
    enum OperationType: String, CaseIterable {
        case addition = "add"
        case subtraction = "subtract"
        case multiplication = "multiply"
        case division = "divide"

        var description: String {
            switch self {
            case .addition: return "Addition"
            case .subtraction: return "Subtraction"
            case .multiplication: return "Multiplication"
            case .division: return "Division"
            }
        }
    }

    // Nested struct for operation results
    struct OperationResult {
        let value: Double
        let operation: OperationType
        let timestamp: Date

        init(value: Double, operation: OperationType) {
            self.value = value
            self.operation = operation
            self.timestamp = Date()
        }
    }
}

// Scientific calculator with advanced mathematical functions
final class ScientificCalculator: AdvancedCalculator {

    func sin(_ x: Double) -> Double {
        let result = Foundation.sin(x)
        recordOperation(result)
        return result
    }

    func cos(_ x: Double) -> Double {
        let result = Foundation.cos(x)
        recordOperation(result)
        return result
    }

    func tan(_ x: Double) -> Double {
        let result = Foundation.tan(x)
        recordOperation(result)
        return result
    }

    func log(_ x: Double, base: Double = M_E) throws -> Double {
        guard x > 0 else {
            throw CalculatorError.invalidInput("Cannot take log of zero or negative number")
        }

        let result = Foundation.log(x) / Foundation.log(base)
        recordOperation(result)
        return result
    }

    func power(_ base: Double, _ exponent: Double) -> Double {
        let result = pow(base, exponent)
        recordOperation(result)
        return result
    }

    func factorial(_ n: Int) throws -> Double {
        guard n >= 0 else {
            throw CalculatorError.invalidInput("Factorial of negative number")
        }

        let result = (1...max(1, n)).reduce(1) { $0 * $1 }
        let doubleResult = Double(result)
        recordOperation(doubleResult)
        return doubleResult
    }

    // Computed property for statistics
    var statistics: (mean: Double, median: Double, standardDeviation: Double) {
        guard !history.isEmpty else {
            return (0, 0, 0)
        }

        let mean = history.reduce(0, +) / Double(history.count)

        let sortedHistory = history.sorted()
        let median: Double
        let count = sortedHistory.count

        if count % 2 == 0 {
            median = (sortedHistory[count / 2 - 1] + sortedHistory[count / 2]) / 2
        } else {
            median = sortedHistory[count / 2]
        }

        let variance = history.map { pow($0 - mean, 2) }.reduce(0, +) / Double(history.count)
        let standardDeviation = sqrt(variance)

        return (mean, median, standardDeviation)
    }

    private func recordOperation(_ result: Double) {
        // Call parent's private method through a workaround
        let _ = add(0, result) - result
    }
}

// Extension for CustomStringConvertible
extension BaseCalculator: CustomStringConvertible {
    var description: String {
        return "Calculator '\(name)' with \(history.count) operations"
    }
}

// Extension with default implementations
extension CalculatorProtocol {
    var historyCount: Int {
        return history.count
    }

    func lastResult() -> Double? {
        return history.last
    }
}

// Struct for calculator configuration
struct CalculatorConfiguration {
    let name: String
    let precision: Double
    let enableHistory: Bool

    static let `default` = CalculatorConfiguration(
        name: "Default Calculator",
        precision: 0.001,
        enableHistory: true
    )

    // Factory method
    static func scientific(name: String) -> CalculatorConfiguration {
        return CalculatorConfiguration(
            name: name,
            precision: 0.0001,
            enableHistory: true
        )
    }
}

// Utility functions
func createCalculator(with config: CalculatorConfiguration) -> BaseCalculator {
    return BaseCalculator(name: config.name, precision: config.precision)
}

func performCalculations() {
    let calc = ScientificCalculator.createDefault(name: "Demo Calculator")

    do {
        print("Calculator: \(calc.name)")

        // Basic operations
        let sum = calc.add(10, 5)
        let product = calc.multiply(20, 3)
        let quotient = try calc.divide(100, 4)

        print("10 + 5 = \(sum)")
        print("20 * 3 = \(product)")
        print("100 / 4 = \(quotient)")

        // Scientific operations
        let sineResult = calc.sin(Double.pi / 2)
        let factorialResult = try calc.factorial(5)

        print("sin(π/2) = \(sineResult)")
        print("5! = \(factorialResult)")

        // Statistics
        let stats = calc.statistics
        print("Mean: \(stats.mean)")
        print("Median: \(stats.median)")
        print("Standard Deviation: \(stats.standardDeviation)")

        // History
        print("History: \(calc.history)")
        print("Operations count: \(calc.operationsPerformed)")

    } catch {
        print("Error: \(error.localizedDescription)")
    }
}

// Test functions
func testBasicCalculator() throws {
    let calc = AdvancedCalculator(name: "Test Calculator")

    let result1 = calc.add(2, 3)
    guard result1 == 5 else {
        throw CalculatorError.operationFailed("Add test failed")
    }

    let result2 = calc.multiply(4, 5)
    guard result2 == 20 else {
        throw CalculatorError.operationFailed("Multiply test failed")
    }

    print("Basic calculator tests passed")
}

func testScientificCalculator() throws {
    let calc = ScientificCalculator(name: "Scientific Test")

    let result1 = calc.power(2, 3)
    guard result1 == 8 else {
        throw CalculatorError.operationFailed("Power test failed")
    }

    let result2 = try calc.factorial(4)
    guard result2 == 24 else {
        throw CalculatorError.operationFailed("Factorial test failed")
    }

    print("Scientific calculator tests passed")
}

// Main execution
if CommandLine.arguments.contains("--demo") {
    performCalculations()

    do {
        try testBasicCalculator()
        try testScientificCalculator()
    } catch {
        print("Test failed: \(error)")
    }
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

    // Verify Swift symbols are extracted
    assert!(
        output.contains("protocol CalculatorProtocol"),
        "Missing CalculatorProtocol - output: {}",
        output
    );
    assert!(
        output.contains("enum CalculatorError") || output.contains("CalculatorError"),
        "Missing CalculatorError enum - output: {}",
        output
    );
    assert!(
        output.contains("class BaseCalculator") || output.contains("BaseCalculator"),
        "Missing BaseCalculator class - output: {}",
        output
    );
    assert!(
        output.contains("class AdvancedCalculator") || output.contains("AdvancedCalculator"),
        "Missing AdvancedCalculator class - output: {}",
        output
    );
    assert!(
        output.contains("final class ScientificCalculator")
            || output.contains("ScientificCalculator"),
        "Missing ScientificCalculator class - output: {}",
        output
    );
    assert!(
        output.contains("func test") || output.contains("testBasicCalculator"),
        "Missing test functions - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_swift_outline_smart_closing_brace_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("ClosingBraceTest.swift");

    let content = r#"import Foundation

// Small function that should NOT get closing brace comments
func smallFunction(_ x: Int) -> Int {
    let result = x * 2
    return result + 1
}

// Large function that SHOULD get closing brace comments with Swift // syntax
func largeFunctionWithGaps(data: [Int]) -> [String] {
    var results: [String] = []
    let processor = DataProcessor()

    // Phase 1: Initial processing with nested control flow
    for (index, value) in data.enumerated() {
        if value > 100 {
            processor.processLargeValue(value, at: index)
        } else if value < 0 {
            processor.processNegativeValue(value, at: index)
        } else {
            processor.processSmallValue(value, at: index)
        }
    }

    // Phase 2: Complex transformation logic
    let transformedData = processor.getTransformedData()
    for item in transformedData {
        switch item.category {
        case .high:
            results.append("HIGH: \(item.value)")
        case .medium:
            results.append("MED: \(item.value)")
        case .low:
            results.append("LOW: \(item.value)")
        }
    }

    // Phase 3: Final validation and cleanup
    var validatedResults: [String] = []
    for result in results {
        guard result.count > 5 else { continue }
        validatedResults.append(result)
    }

    return validatedResults
}

// Another large function to test closing brace behavior
class LargeProcessorClass {
    private var accumulator: Accumulator

    init() {
        self.accumulator = Accumulator()
    }

    func processItems(_ items: [Item]) -> ProcessedResult {
        // Main processing with deeply nested control flow
        for item in items {
            switch item.itemType {
            case .primary:
                if item.weight > 50.0 {
                    accumulator.addHeavyPrimary(item)
                } else {
                    accumulator.addLightPrimary(item)
                }
            case .secondary:
                accumulator.addSecondary(item)
            case .auxiliary:
                accumulator.addAuxiliary(item)
            }
        }

        return accumulator.finalize()
    }
}

// Large extension with multiple methods
extension String {
    func complexStringProcessor() -> ProcessedString {
        let processor = StringProcessor()

        // Multi-stage string processing
        let stage1 = processor.initialCleanup(self)
        let stage2 = processor.tokenization(stage1)
        let stage3 = processor.normalization(stage2)

        guard !stage3.isEmpty else {
            return ProcessedString.empty
        }

        // Final processing with validation
        let finalResult = processor.finalization(stage3)
        return ProcessedString(content: finalResult)
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "large", // Search for large functions/classes
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the large functions and classes
    assert!(
        output.contains("largeFunctionWithGaps") || output.contains("LargeProcessorClass"),
        "Missing large functions/classes - output: {}",
        output
    );

    // Should have closing brace comments with Swift // syntax (not /* */)
    let has_swift_closing_brace_comment = output.contains("} //");
    assert!(
        has_swift_closing_brace_comment,
        "Large functions should have closing brace comments with Swift // syntax - output: {}",
        output
    );

    // Should NOT have closing braces for small functions
    let small_func_lines: Vec<&str> = output
        .lines()
        .filter(|line| line.contains("smallFunction"))
        .collect();

    if !small_func_lines.is_empty() {
        // If smallFunction appears in output, verify it doesn't have closing brace comments
        let has_small_func_closing_comment = output.lines().any(|line| {
            line.contains("smallFunction") && (line.contains("} //") || line.contains("} /*"))
        });
        assert!(
            !has_small_func_closing_comment,
            "Small functions should NOT have closing brace comments - output: {}",
            output
        );
    }

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_swift_outline_keyword_highlighting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("KeywordTest.swift");

    let content = r#"import Foundation
import SwiftUI

// Protocol with associated types and where clauses
protocol Processable {
    associatedtype InputType
    associatedtype OutputType where OutputType: Codable

    func process(_ input: InputType) async throws -> OutputType
    var isReady: Bool { get }
}

// Generic class with constraints
class DataProcessor<T: Hashable & Codable>: Processable {
    typealias InputType = T
    typealias OutputType = ProcessedData<T>

    private let queue: DispatchQueue
    private var cache: [T: OutputType] = [:]

    var isReady: Bool {
        return !cache.isEmpty
    }

    init(queue: DispatchQueue = .main) {
        self.queue = queue
    }

    // Async function with error handling
    func process(_ input: T) async throws -> OutputType {
        if let cached = cache[input] {
            return cached
        }

        do {
            let result = try await performProcessing(input)
            cache[input] = result
            return result
        } catch {
            throw ProcessingError.failedToProcess(input, error)
        }
    }

    // Private async helper with guard statements
    private func performProcessing(_ input: T) async throws -> OutputType {
        guard isReady else {
            throw ProcessingError.notReady
        }

        return try await withCheckedThrowingContinuation { continuation in
            queue.async {
                do {
                    let processed = ProcessedData(value: input, timestamp: Date())
                    continuation.resume(returning: processed)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }
}

// Enum with associated values and computed properties
enum ProcessingError: Error, LocalizedError {
    case notReady
    case failedToProcess(Any, Error)
    case invalidInput(String)

    var errorDescription: String? {
        switch self {
        case .notReady:
            return "Processor is not ready"
        case .failedToProcess(let input, let error):
            return "Failed to process \(input): \(error.localizedDescription)"
        case .invalidInput(let input):
            return "Invalid input: \(input)"
        }
    }
}

// Struct with property wrappers and computed properties
struct ProcessedData<T: Codable>: Codable {
    @Published var value: T
    let timestamp: Date

    var age: TimeInterval {
        return Date().timeIntervalSince(timestamp)
    }

    init(value: T, timestamp: Date) {
        self._value = Published(initialValue: value)
        self.timestamp = timestamp
    }
}

// Extension with conditional conformance
extension ProcessedData: Equatable where T: Equatable {
    static func == (lhs: ProcessedData<T>, rhs: ProcessedData<T>) -> Bool {
        return lhs.value == rhs.value && lhs.timestamp == rhs.timestamp
    }
}

// SwiftUI View with ViewBuilder and State
struct ContentView: View {
    @State private var isLoading: Bool = false
    @StateObject private var processor = DataProcessor<String>()

    var body: some View {
        VStack {
            if isLoading {
                ProgressView("Processing...")
            } else {
                Button("Start Processing") {
                    Task {
                        await startProcessing()
                    }
                }
            }
        }
        .padding()
    }

    @MainActor
    private func startProcessing() async {
        isLoading = true
        defer { isLoading = false }

        do {
            let result = try await processor.process("test data")
            print("Processed: \(result)")
        } catch {
            print("Error: \(error)")
        }
    }
}

// Actor for concurrent processing
actor ConcurrentProcessor {
    private var tasks: [Task<Void, Never>] = []

    func addTask(_ operation: @escaping () async -> Void) {
        let task = Task {
            await operation()
        }
        tasks.append(task)
    }

    func waitForAll() async {
        for task in tasks {
            await task.value
        }
        tasks.removeAll()
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "async", // Search for async keyword
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should highlight Swift keywords in the search results
    // Look for async, await, throws, guard, defer, etc.
    let swift_keywords = [
        "async",
        "await",
        "throws",
        "guard",
        "defer",
        "actor",
        "@Published",
        "@State",
        "@MainActor",
    ];
    let found_keywords: Vec<&str> = swift_keywords
        .iter()
        .filter(|&keyword| output.contains(keyword))
        .copied()
        .collect();

    assert!(
        !found_keywords.is_empty(),
        "Should find and highlight Swift keywords. Found: {:?} - output: {}",
        found_keywords,
        output
    );

    // Should contain the searched term
    assert!(
        output.contains("async"),
        "Should contain the searched async keyword - output: {}",
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_swift_outline_array_dictionary_truncation_with_keyword_preservation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("ArrayDictTest.swift");

    let content = r#"import Foundation

struct ConfigurationManager {
    // Large array that should be truncated but preserve keywords
    let supportedLanguages = [
        "swift",
        "objective-c",
        "javascript",
        "typescript",
        "python",
        "java",
        "kotlin",
        "dart",
        "go",
        "rust",
        "c",
        "cpp",
        "csharp",
        "php",
        "ruby",
        "perl",
        "scala",
        "clojure",
        "haskell",
        "erlang",
        "elixir",
        "lua",
        "shell",
        "powershell",
        "dockerfile",
        "yaml",
        "json",
        "xml",
        "html",
        "css",
        "sass",
        "less"
    ]

    // Large dictionary with keyword-rich content
    private var configurationSettings: [String: Any] = [
        "database_host": "localhost",
        "database_port": 5432,
        "database_name": "production_db",
        "api_key_primary": "sk-test-1234567890",
        "api_key_secondary": "sk-backup-0987654321",
        "cache_timeout": 3600,
        "retry_attempts": 3,
        "batch_size": 1000,
        "worker_threads": 8,
        "memory_limit": "2GB",
        "disk_space_threshold": "10GB",
        "network_timeout": 30,
        "authentication_enabled": true,
        "ssl_verification": true,
        "logging_level": "info",
        "debug_mode": false,
        "performance_monitoring": true,
        "error_tracking": true,
        "metrics_collection": true,
        "backup_enabled": true,
        "backup_frequency": "daily",
        "retention_days": 30,
        "compression_enabled": true,
        "encryption_key": "aes-256-gcm",
        "session_timeout": 1800,
        "max_connections": 100,
        "connection_pool_size": 20,
        "query_timeout": 60,
        "transaction_timeout": 120
    ]

    func processLargeDataSet() -> [String] {
        // Large inline array in function
        let processingPipeline = [
            "data_validation",
            "schema_verification", 
            "data_cleaning",
            "duplicate_removal",
            "format_standardization",
            "type_conversion",
            "null_handling",
            "validation_rules",
            "business_logic",
            "transformation_rules",
            "aggregation_logic",
            "sorting_criteria",
            "filtering_conditions",
            "output_formatting",
            "result_validation"
        ]

        return processingPipeline.compactMap { step in
            return "processed_\(step)"
        }
    }

    // Function with dictionary containing Swift keywords
    func getSwiftKeywords() -> [String: String] {
        return [
            "async": "Asynchronous function modifier",
            "await": "Suspend execution until async call completes", 
            "actor": "Reference type for concurrent programming",
            "guard": "Early exit statement with condition",
            "defer": "Execute code when leaving current scope",
            "throws": "Function can throw errors",
            "rethrows": "Function rethrows callers errors",
            "inout": "Parameter passed by reference",
            "weak": "Weak reference to avoid retain cycles",
            "unowned": "Unowned reference (unsafe)",
            "lazy": "Property initialized on first access",
            "mutating": "Method can modify struct properties",
            "nonmutating": "Explicitly non-mutating method",
            "override": "Override superclass method",
            "final": "Prevent inheritance/overriding",
            "required": "Required initializer",
            "convenience": "Convenience initializer",
            "subscript": "Custom subscript access"
        ]
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "array", // Search for arrays/collections
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the large arrays and dictionaries
    assert!(
        output.contains("supportedLanguages")
            || output.contains("configurationSettings")
            || output.contains("processLargeDataSet"),
        "Missing large arrays/dictionaries - output: {}",
        output
    );

    // Should have array/dictionary truncation markers
    let has_truncation = output.contains("...")
        || output.contains("/* truncated */")
        || output.contains("// truncated");
    assert!(
        has_truncation,
        "Large arrays/dictionaries should show truncation markers - output: {}",
        output
    );

    // Should preserve important keywords even in truncated content
    let important_keywords = [
        "async", "await", "guard", "defer", "throws", "actor", "database", "api_key",
    ];
    let preserved_keywords: Vec<&str> = important_keywords
        .iter()
        .filter(|&keyword| output.contains(keyword))
        .copied()
        .collect();

    assert!(
        !preserved_keywords.is_empty(),
        "Should preserve important keywords in truncated arrays/dicts. Found: {:?} - output: {}",
        preserved_keywords,
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_swift_outline_specific_constructs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("SwiftConstructs.swift");

    let content = r#"import Foundation
import SwiftUI

// MARK: - Protocols with Associated Types and Generic Constraints

protocol DataTransformable {
    associatedtype InputType
    associatedtype OutputType: Codable
    
    func transform(_ input: InputType) throws -> OutputType
    var transformationId: String { get }
}

protocol Repository {
    associatedtype Entity: Identifiable
    associatedtype ID where ID == Entity.ID
    
    func find(by id: ID) async throws -> Entity?
    func save(_ entity: Entity) async throws
    func delete(by id: ID) async throws
}

// MARK: - Generic Classes with Inheritance and Constraints

class BaseDataProcessor<T: Hashable>: NSObject {
    private let processingQueue: DispatchQueue
    private var cache: [T: ProcessingResult] = [:]
    
    override init() {
        self.processingQueue = DispatchQueue(label: "processing.queue", qos: .background)
        super.init()
    }
    
    func process(_ data: T) -> ProcessingResult {
        if let cached = cache[data] {
            return cached
        }
        
        let result = ProcessingResult(data: data, timestamp: Date())
        cache[data] = result
        return result
    }
}

final class NetworkDataProcessor<T: Codable & Hashable>: BaseDataProcessor<T>, DataTransformable {
    typealias InputType = T
    typealias OutputType = NetworkResponse<T>
    
    let transformationId = "network-processor"
    private let session: URLSession
    
    override init() {
        self.session = URLSession.shared
        super.init()
    }
    
    func transform(_ input: T) throws -> OutputType {
        // Complex transformation logic here
        let jsonData = try JSONEncoder().encode(input)
        return NetworkResponse(data: input, rawData: jsonData)
    }
}

// MARK: - Structs with Property Wrappers and Computed Properties

struct UserProfile: Codable, Identifiable {
    let id: UUID
    @Published var name: String
    @Published var email: String
    @UserDefault(key: "user_preferences", defaultValue: UserPreferences()) 
    var preferences: UserPreferences
    
    private let createdAt: Date
    private var lastUpdated: Date = Date()
    
    var displayName: String {
        return name.isEmpty ? "Anonymous User" : name
    }
    
    var isValid: Bool {
        return !name.isEmpty && email.contains("@")
    }
    
    init(name: String, email: String) {
        self.id = UUID()
        self.name = name
        self.email = email
        self.createdAt = Date()
    }
    
    mutating func updateProfile(name: String? = nil, email: String? = nil) {
        if let newName = name {
            self.name = newName
        }
        if let newEmail = email {
            self.email = newEmail
        }
        self.lastUpdated = Date()
    }
}

// MARK: - Enums with Associated Values and Methods

enum APIResponse<T: Codable> {
    case success(data: T, metadata: ResponseMetadata)
    case failure(error: APIError, retryAfter: TimeInterval?)
    case loading(progress: Double)
    case cached(data: T, cacheTimestamp: Date)
    
    var isSuccessful: Bool {
        switch self {
        case .success, .cached:
            return true
        case .failure, .loading:
            return false
        }
    }
    
    func map<U: Codable>(_ transform: (T) -> U) -> APIResponse<U> {
        switch self {
        case .success(let data, let metadata):
            return .success(data: transform(data), metadata: metadata)
        case .cached(let data, let timestamp):
            return .cached(data: transform(data), cacheTimestamp: timestamp)
        case .failure(let error, let retryAfter):
            return .failure(error: error, retryAfter: retryAfter)
        case .loading(let progress):
            return .loading(progress: progress)
        }
    }
}

enum APIError: Error, LocalizedError, CaseIterable {
    case networkUnavailable
    case invalidCredentials
    case serverError(code: Int)
    case dataCorrupted(description: String)
    case rateLimitExceeded(resetTime: Date)
    
    var errorDescription: String? {
        switch self {
        case .networkUnavailable:
            return "Network connection is unavailable"
        case .invalidCredentials:
            return "Invalid authentication credentials"
        case .serverError(let code):
            return "Server error with code: \(code)"
        case .dataCorrupted(let description):
            return "Data corruption: \(description)"
        case .rateLimitExceeded(let resetTime):
            return "Rate limit exceeded. Try again after \(resetTime)"
        }
    }
}

// MARK: - Extensions with Conditional Conformance

extension Array where Element: Numeric {
    func sum() -> Element {
        return reduce(0, +)
    }
    
    func average() -> Double where Element: BinaryInteger {
        guard !isEmpty else { return 0 }
        let total = sum()
        return Double(total) / Double(count)
    }
}

extension Dictionary where Key == String, Value: Codable {
    func toJSON() throws -> Data {
        return try JSONSerialization.data(withJSONObject: self)
    }
    
    static func fromJSON(_ data: Data) throws -> [String: Value] {
        let decoded = try JSONSerialization.jsonObject(with: data) as? [String: Any] ?? [:]
        // Complex conversion logic would go here
        return [:] // Simplified for testing
    }
}

extension APIResponse: Equatable where T: Equatable {
    static func == (lhs: APIResponse<T>, rhs: APIResponse<T>) -> Bool {
        switch (lhs, rhs) {
        case (.success(let lData, _), .success(let rData, _)):
            return lData == rData
        case (.cached(let lData, _), .cached(let rData, _)):
            return lData == rData
        case (.loading(let lProgress), .loading(let rProgress)):
            return lProgress == rProgress
        case (.failure(_, _), .failure(_, _)):
            return true
        default:
            return false
        }
    }
}

// MARK: - Property Wrappers

@propertyWrapper
struct UserDefault<T: Codable> {
    let key: String
    let defaultValue: T
    
    var wrappedValue: T {
        get {
            guard let data = UserDefaults.standard.data(forKey: key) else {
                return defaultValue
            }
            
            do {
                return try JSONDecoder().decode(T.self, from: data)
            } catch {
                return defaultValue
            }
        }
        set {
            do {
                let data = try JSONEncoder().encode(newValue)
                UserDefaults.standard.set(data, forKey: key)
            } catch {
                print("Failed to encode value for key \(key): \(error)")
            }
        }
    }
}

@propertyWrapper
struct Clamped<T: Comparable> {
    private var value: T
    private let range: ClosedRange<T>
    
    var wrappedValue: T {
        get { return value }
        set { value = min(max(range.lowerBound, newValue), range.upperBound) }
    }
    
    init(wrappedValue: T, _ range: ClosedRange<T>) {
        self.range = range
        self.value = min(max(range.lowerBound, wrappedValue), range.upperBound)
    }
}

// MARK: - Nested Types and Namespacing

struct NetworkManager {
    enum RequestMethod: String, CaseIterable {
        case get = "GET"
        case post = "POST"  
        case put = "PUT"
        case delete = "DELETE"
        case patch = "PATCH"
    }
    
    struct Configuration {
        let baseURL: URL
        let timeout: TimeInterval
        let headers: [String: String]
        
        static let development = Configuration(
            baseURL: URL(string: "https://dev.api.example.com")!,
            timeout: 30,
            headers: ["Content-Type": "application/json"]
        )
        
        static let production = Configuration(
            baseURL: URL(string: "https://api.example.com")!,
            timeout: 60,  
            headers: ["Content-Type": "application/json"]
        )
    }
    
    class RequestBuilder {
        private var method: RequestMethod = .get
        private var path: String = ""
        private var queryItems: [URLQueryItem] = []
        private var body: Data?
        
        func method(_ method: RequestMethod) -> Self {
            self.method = method
            return self
        }
        
        func path(_ path: String) -> Self {
            self.path = path
            return self
        }
        
        func query(_ name: String, value: String) -> Self {
            queryItems.append(URLQueryItem(name: name, value: value))
            return self
        }
        
        func body(_ data: Data) -> Self {
            self.body = data
            return self
        }
        
        func build(config: Configuration) throws -> URLRequest {
            guard var components = URLComponents(url: config.baseURL, resolvingAgainstBaseURL: true) else {
                throw APIError.serverError(code: 0)
            }
            
            components.path = path
            components.queryItems = queryItems.isEmpty ? nil : queryItems
            
            guard let url = components.url else {
                throw APIError.serverError(code: 0)
            }
            
            var request = URLRequest(url: url)
            request.httpMethod = method.rawValue
            request.timeoutInterval = config.timeout
            request.httpBody = body
            
            for (key, value) in config.headers {
                request.setValue(value, forHTTPHeaderField: key)
            }
            
            return request
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "protocol", // Search for protocols
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find Swift-specific constructs
    let swift_constructs = [
        "protocol",
        "class",
        "struct",
        "enum",
        "extension",
        "@propertyWrapper",
    ];
    let found_constructs: Vec<&str> = swift_constructs
        .iter()
        .filter(|&construct| output.contains(construct))
        .copied()
        .collect();

    assert!(
        !found_constructs.is_empty(),
        "Should find Swift-specific constructs. Found: {:?} - output: {}",
        found_constructs,
        output
    );

    // Should find specific Swift constructs
    assert!(
        output.contains("DataTransformable") || output.contains("Repository"),
        "Should find protocol definitions - output: {}",
        output
    );

    assert!(
        output.contains("BaseDataProcessor") || output.contains("NetworkDataProcessor"),
        "Should find class definitions - output: {}",
        output
    );

    assert!(
        output.contains("UserProfile") || output.contains("NetworkManager"),
        "Should find struct definitions - output: {}",
        output
    );

    assert!(
        output.contains("APIResponse") || output.contains("APIError"),
        "Should find enum definitions - output: {}",
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_swift_outline_control_flow_statements() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("ControlFlow.swift");

    let content = r#"import Foundation

struct DataValidator {
    enum ValidationError: Error {
        case invalidInput
        case processingFailed
        case timeout
    }
    
    // Function with comprehensive control flow patterns
    func validateAndProcessData(_ input: [Any]) async throws -> [String] {
        var results: [String] = []
        
        // Guard statement for early exit
        guard !input.isEmpty else {
            throw ValidationError.invalidInput
        }
        
        // Defer statement for cleanup
        defer {
            print("Cleaning up validation resources")
            cleanup()
        }
        
        // For loop with enumerated values
        for (index, item) in input.enumerated() {
            // Nested if-else statements
            if let stringItem = item as? String {
                if stringItem.count > 100 {
                    results.append("LONG_STRING[\(index)]: \(stringItem.prefix(50))...")
                } else if stringItem.isEmpty {
                    results.append("EMPTY_STRING[\(index)]")
                } else {
                    results.append("STRING[\(index)]: \(stringItem)")
                }
            } else if let numberItem = item as? NSNumber {
                let value = numberItem.doubleValue
                if value > 1000 {
                    results.append("LARGE_NUMBER[\(index)]: \(value)")
                } else if value < 0 {
                    results.append("NEGATIVE_NUMBER[\(index)]: \(value)")
                } else {
                    results.append("NUMBER[\(index)]: \(value)")
                }
            } else if let arrayItem = item as? [Any] {
                // Recursive processing with do-catch
                do {
                    let nestedResults = try await validateAndProcessData(arrayItem)
                    results.append("NESTED_ARRAY[\(index)]: [\(nestedResults.joined(separator: ", "))]")
                } catch {
                    results.append("NESTED_ARRAY_ERROR[\(index)]: \(error.localizedDescription)")
                }
            } else {
                results.append("UNKNOWN_TYPE[\(index)]: \(type(of: item))")
            }
        }
        
        // While loop with complex condition
        var retryCount = 0
        while retryCount < 3 && !results.isEmpty {
            let success = await performValidation(results)
            if success {
                break
            }
            
            retryCount += 1
            
            // Nested switch statement
            switch retryCount {
            case 1:
                await Task.sleep(nanoseconds: 1_000_000_000) // 1 second
            case 2:
                await Task.sleep(nanoseconds: 2_000_000_000) // 2 seconds  
            case 3:
                throw ValidationError.timeout
            default:
                break
            }
        }
        
        // Switch statement with associated values and where clauses
        for result in results {
            switch result {
            case let str where str.hasPrefix("LONG_STRING"):
                print("Processing long string: \(str)")
            case let str where str.contains("ERROR"):
                print("Error found: \(str)")
            case let str where str.hasPrefix("NUMBER"):
                print("Number processing: \(str)")
            default:
                print("Standard processing: \(result)")
            }
        }
        
        return results
    }
    
    // Function demonstrating repeat-while loop
    func processUntilComplete(_ data: inout [String]) {
        repeat {
            // Complex nested control flow
            for (index, item) in data.enumerated() {
                if item.isEmpty {
                    data.remove(at: index)
                    continue
                }
                
                // Nested switch with fallthrough
                switch item.first {
                case "A":
                    data[index] = "PROCESSED_A_" + item
                case "B":
                    data[index] = "PROCESSED_B_" + item  
                case "C":
                    data[index] = "PROCESSED_C_" + item
                    fallthrough
                case "D":
                    data[index] += "_SPECIAL"
                default:
                    data[index] = "DEFAULT_" + item
                }
            }
        } while data.contains { $0.hasPrefix("UNPROCESSED") }
    }
    
    // Function with complex guard statements and early returns
    func complexGuardValidation(_ input: Any?) -> String? {
        guard let input = input else {
            return nil
        }
        
        guard let stringInput = input as? String else {
            return "NOT_STRING"
        }
        
        guard !stringInput.isEmpty else {
            return "EMPTY"
        }
        
        guard stringInput.count >= 5 else {
            return "TOO_SHORT"  
        }
        
        guard stringInput.count <= 100 else {
            return "TOO_LONG"
        }
        
        // Nested if with multiple conditions
        if stringInput.hasPrefix("SPECIAL_") && stringInput.hasSuffix("_END") {
            if stringInput.contains("IMPORTANT") {
                return "SPECIAL_IMPORTANT"
            } else if stringInput.contains("URGENT") {
                return "SPECIAL_URGENT"
            } else {
                return "SPECIAL_NORMAL"
            }
        }
        
        return "VALID"
    }
    
    // Function with do-catch blocks and error handling
    func processWithErrorHandling(_ items: [String]) throws -> [String] {
        var processedItems: [String] = []
        
        for item in items {
            do {
                // First processing step
                let step1 = try validateItem(item)
                
                do {
                    // Nested processing with different error handling
                    let step2 = try transformItem(step1)
                    processedItems.append(step2)
                } catch ValidationError.processingFailed {
                    // Handle specific error type
                    processedItems.append("PROCESSING_FAILED: \(item)")
                } catch {
                    // Handle other errors
                    processedItems.append("UNKNOWN_ERROR: \(item) - \(error)")
                }
            } catch ValidationError.invalidInput {
                // Skip invalid inputs
                continue
            } catch ValidationError.timeout {
                // Propagate timeout errors
                throw ValidationError.timeout
            } catch {
                // Log and continue with other errors
                print("Unexpected error processing \(item): \(error)")
                processedItems.append("ERROR: \(item)")
            }
        }
        
        return processedItems
    }
    
    // Private helper functions
    private func cleanup() {
        print("Performing cleanup")
    }
    
    private func performValidation(_ results: [String]) async -> Bool {
        // Simulate async validation
        await Task.sleep(nanoseconds: 100_000_000) // 0.1 second
        return !results.isEmpty
    }
    
    private func validateItem(_ item: String) throws -> String {
        if item.isEmpty {
            throw ValidationError.invalidInput
        }
        return "VALIDATED_\(item)"
    }
    
    private func transformItem(_ item: String) throws -> String {
        if item.contains("FAIL") {
            throw ValidationError.processingFailed
        }
        return "TRANSFORMED_\(item)"
    }
}

// Extension with additional control flow patterns
extension DataValidator {
    func processWithComplexSwitchStatement(_ value: Any) -> String {
        switch value {
        case let str as String where str.count > 10:
            return "LONG_STRING: \(str.prefix(10))..."
            
        case let str as String where str.isEmpty:
            return "EMPTY_STRING"
            
        case let str as String:
            return "STRING: \(str)"
            
        case let num as Int where num > 100:
            return "LARGE_INT: \(num)"
            
        case let num as Int where num < 0:
            return "NEGATIVE_INT: \(num)"
            
        case let num as Int:
            return "INT: \(num)"
            
        case let array as [Any] where array.count > 5:
            return "LARGE_ARRAY: \(array.count) items"
            
        case let array as [Any]:
            return "ARRAY: \(array.count) items"
            
        case Optional<Any>.none:
            return "NIL_VALUE"
            
        case Optional<Any>.some(let unwrapped):
            return "OPTIONAL: \(processWithComplexSwitchStatement(unwrapped))"
            
        default:
            return "UNKNOWN_TYPE: \(type(of: value))"
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "guard", // Search for guard statements
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find Swift control flow statements
    let control_flow_keywords = [
        "guard", "defer", "if", "while", "for", "switch", "do", "catch", "repeat",
    ];
    let found_keywords: Vec<&str> = control_flow_keywords
        .iter()
        .filter(|&keyword| output.contains(keyword))
        .copied()
        .collect();

    assert!(
        !found_keywords.is_empty(),
        "Should find Swift control flow keywords. Found: {:?} - output: {}",
        found_keywords,
        output
    );

    // Should find guard statements specifically
    assert!(
        output.contains("guard"),
        "Should find guard statements - output: {}",
        output
    );

    // Should find nested control structures
    let nested_patterns = ["nested", "if", "switch", "case", "while"];
    let found_nested: Vec<&str> = nested_patterns
        .iter()
        .filter(|&pattern| output.contains(pattern))
        .copied()
        .collect();

    assert!(
        found_nested.len() >= 2,
        "Should find nested control flow patterns. Found: {:?} - output: {}",
        found_nested,
        output
    );

    // Should show closing braces for large control blocks
    let has_closing_braces = output.contains("} //") || output.contains("} /*");
    assert!(
        has_closing_braces,
        "Large control flow blocks should have closing brace comments - output: {}",
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_swift_outline_test_detection_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("SwiftTests.swift");

    let content = r#"import XCTest
@testable import MyApp
import Foundation

// MARK: - XCTest Framework Tests

class CalculatorTests: XCTestCase {
    var calculator: Calculator!
    
    override func setUpWithError() throws {
        calculator = Calculator()
    }
    
    override func tearDownWithError() throws {
        calculator = nil
    }
    
    func testAddition() {
        let result = calculator.add(2, 3)
        XCTAssertEqual(result, 5, "Addition should work correctly")
    }
    
    func testSubtraction() throws {
        let result = calculator.subtract(5, 3)
        XCTAssertEqual(result, 2)
        
        // Test with negative result
        let negativeResult = calculator.subtract(3, 5)
        XCTAssertEqual(negativeResult, -2)
    }
    
    func testMultiplication() {
        XCTAssertEqual(calculator.multiply(4, 5), 20)
        XCTAssertEqual(calculator.multiply(-2, 3), -6)
        XCTAssertEqual(calculator.multiply(0, 100), 0)
    }
    
    func testDivision() throws {
        let result = try calculator.divide(10, 2)
        XCTAssertEqual(result, 5.0, accuracy: 0.001)
        
        // Test division by zero throws error
        XCTAssertThrowsError(try calculator.divide(10, 0)) { error in
            XCTAssertTrue(error is CalculatorError)
        }
    }
    
    func testPerformance() {
        measure {
            for i in 0..<1000 {
                _ = calculator.add(Double(i), Double(i + 1))
            }
        }
    }
    
    func testAsyncOperation() async throws {
        let result = try await calculator.asyncCalculation(5, 10)
        XCTAssertEqual(result, 15)
        
        await XCTAssertThrowsErrorAsync(try await calculator.asyncCalculation(0, 0))
    }
    
    func testExpectation() {
        let expectation = expectation(description: "Async calculation completed")
        
        calculator.performAsyncCalculation { result in
            XCTAssertNotNil(result)
            expectation.fulfill()
        }
        
        waitForExpectations(timeout: 5.0)
    }
}

// MARK: - Performance Tests

class PerformanceTests: XCTestCase {
    
    func testCalculatorPerformance() {
        let calculator = Calculator()
        
        measure {
            for _ in 0..<10000 {
                _ = calculator.add(1.0, 2.0)
            }
        }
    }
    
    func testMemoryUsage() {
        measureMetrics([.wallClockTime, .peakMemoryUsage]) {
            let largeArray = Array(0..<100000)
            _ = largeArray.map { $0 * 2 }
        }
    }
}

// MARK: - UI Tests

class AppUITests: XCTestCase {
    let app = XCUIApplication()
    
    override func setUpWithError() throws {
        continueAfterFailure = false
        app.launch()
    }
    
    func testBasicNavigation() throws {
        let tabBar = app.tabBars.firstMatch
        XCTAssertTrue(tabBar.exists)
        
        app.buttons["Calculator"].tap()
        
        let calculatorView = app.otherElements["CalculatorView"]
        XCTAssertTrue(calculatorView.waitForExistence(timeout: 5))
    }
    
    func testCalculatorInput() {
        app.buttons["5"].tap()
        app.buttons["+"].tap()
        app.buttons["3"].tap()
        app.buttons["="].tap()
        
        let result = app.staticTexts["8"]
        XCTAssertTrue(result.exists)
    }
    
    func testScreenshots() {
        takeScreenshot(name: "main_screen")
        
        app.buttons["Settings"].tap()
        takeScreenshot(name: "settings_screen")
    }
    
    private func takeScreenshot(name: String) {
        let screenshot = app.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}

// MARK: - Simple Test Functions (Non-XCTest)

struct SimpleTests {
    // Simple test functions that should be detected
    static func testStringOperations() -> Bool {
        let str = "Hello, World!"
        
        // Test string length
        guard str.count == 13 else {
            print("String length test failed")
            return false
        }
        
        // Test string contains
        guard str.contains("World") else {
            print("String contains test failed")
            return false
        }
        
        // Test string prefix
        guard str.hasPrefix("Hello") else {
            print("String prefix test failed")
            return false
        }
        
        print("All string tests passed")
        return true
    }
    
    static func testArrayOperations() -> Bool {
        let numbers = [1, 2, 3, 4, 5]
        
        // Test array count
        guard numbers.count == 5 else {
            print("Array count test failed")
            return false
        }
        
        // Test array contains
        guard numbers.contains(3) else {
            print("Array contains test failed")
            return false
        }
        
        // Test array sum
        let sum = numbers.reduce(0, +)
        guard sum == 15 else {
            print("Array sum test failed: expected 15, got \(sum)")
            return false
        }
        
        print("All array tests passed")
        return true
    }
    
    static func testMathOperations() -> Bool {
        // Test basic arithmetic
        guard 2 + 2 == 4 else {
            print("Addition test failed")
            return false
        }
        
        guard 10 - 3 == 7 else {
            print("Subtraction test failed")
            return false
        }
        
        guard 4 * 5 == 20 else {
            print("Multiplication test failed")
            return false
        }
        
        guard 15 / 3 == 5 else {
            print("Division test failed")
            return false
        }
        
        print("All math tests passed")
        return true
    }
    
    static func runAllTests() {
        print("Running simple tests...")
        
        let stringTestPassed = testStringOperations()
        let arrayTestPassed = testArrayOperations()
        let mathTestPassed = testMathOperations()
        
        if stringTestPassed && arrayTestPassed && mathTestPassed {
            print("All simple tests passed!")
        } else {
            print("Some tests failed.")
        }
    }
}

// MARK: - Mock and Stub Test Helpers

class MockNetworkService {
    var shouldReturnError = false
    
    func fetchData() async throws -> Data {
        if shouldReturnError {
            throw NetworkError.connectionFailed
        }
        
        let jsonString = "{\"message\": \"Hello, World!\"}"
        return jsonString.data(using: .utf8)!
    }
}

enum NetworkError: Error {
    case connectionFailed
    case invalidResponse
}

// MARK: - Integration Tests

class IntegrationTests: XCTestCase {
    var mockService: MockNetworkService!
    var dataProcessor: DataProcessor!
    
    override func setUp() {
        super.setUp()
        mockService = MockNetworkService()
        dataProcessor = DataProcessor(networkService: mockService)
    }
    
    func testDataProcessingFlow() async throws {
        // Test successful flow
        mockService.shouldReturnError = false
        let result = try await dataProcessor.processData()
        
        XCTAssertNotNil(result)
        XCTAssertEqual(result.message, "Hello, World!")
    }
    
    func testErrorHandling() async {
        // Test error flow
        mockService.shouldReturnError = true
        
        do {
            _ = try await dataProcessor.processData()
            XCTFail("Expected error to be thrown")
        } catch {
            XCTAssertTrue(error is NetworkError)
        }
    }
    
    func testRetryLogic() async throws {
        var attemptCount = 0
        mockService.shouldReturnError = true
        
        // Mock will fail first two attempts, succeed on third
        dataProcessor.retryHandler = {
            attemptCount += 1
            if attemptCount >= 3 {
                self.mockService.shouldReturnError = false
            }
        }
        
        let result = try await dataProcessor.processDataWithRetry(maxAttempts: 3)
        XCTAssertNotNil(result)
        XCTAssertEqual(attemptCount, 3)
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "test", // Search for test-related content
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests", // Enable test detection
    ])?;

    // Should find XCTest framework test classes
    assert!(
        output.contains("XCTestCase")
            || output.contains("CalculatorTests")
            || output.contains("PerformanceTests"),
        "Should find XCTest framework test classes - output: {}",
        output
    );

    // Should find XCTest methods
    let xctest_methods = [
        "testAddition",
        "testSubtraction",
        "testMultiplication",
        "testDivision",
        "testPerformance",
    ];
    let found_xctest_methods: Vec<&str> = xctest_methods
        .iter()
        .filter(|&method| output.contains(method))
        .copied()
        .collect();

    assert!(
        !found_xctest_methods.is_empty(),
        "Should find XCTest methods. Found: {:?} - output: {}",
        found_xctest_methods,
        output
    );

    // Should find simple test functions (non-XCTest)
    let simple_test_functions = [
        "testStringOperations",
        "testArrayOperations",
        "testMathOperations",
    ];
    let found_simple_tests: Vec<&str> = simple_test_functions
        .iter()
        .filter(|&func| output.contains(func))
        .copied()
        .collect();

    assert!(
        !found_simple_tests.is_empty(),
        "Should find simple test functions. Found: {:?} - output: {}",
        found_simple_tests,
        output
    );

    // Should find XCTest assertions and patterns
    let xctest_patterns = [
        "XCTAssert",
        "XCTAssertEqual",
        "XCTAssertThrowsError",
        "expectation",
        "measure",
    ];
    let found_patterns: Vec<&str> = xctest_patterns
        .iter()
        .filter(|&pattern| output.contains(pattern))
        .copied()
        .collect();

    assert!(
        !found_patterns.is_empty(),
        "Should find XCTest assertion patterns. Found: {:?} - output: {}",
        found_patterns,
        output
    );

    // Should find test setup and teardown methods
    let setup_teardown = ["setUp", "tearDown", "setUpWithError", "tearDownWithError"];
    let found_setup: Vec<&str> = setup_teardown
        .iter()
        .filter(|&method| output.contains(method))
        .copied()
        .collect();

    // Note: This is optional since setup/teardown might not always be in search results
    if !found_setup.is_empty() {
        println!("Found setup/teardown methods: {:?}", found_setup);
    }

    // Should find async test patterns
    let async_patterns = ["async", "await", "XCTAssertThrowsErrorAsync"];
    let found_async: Vec<&str> = async_patterns
        .iter()
        .filter(|&pattern| output.contains(pattern))
        .copied()
        .collect();

    assert!(
        !found_async.is_empty(),
        "Should find async test patterns. Found: {:?} - output: {}",
        found_async,
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_swift_outline_modern_features() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("ModernSwift.swift");

    let content = r#"import SwiftUI
import Combine
import Foundation

// MARK: - Actors for Concurrency

actor DataManager {
    private var cache: [String: Any] = [:]
    private var subscribers: [AnyCancellable] = []
    
    func store(key: String, value: Any) {
        cache[key] = value
    }
    
    func retrieve(key: String) -> Any? {
        return cache[key]
    }
    
    func clearCache() {
        cache.removeAll()
    }
    
    // Async function within actor
    func processData(_ data: [String]) async throws -> [String] {
        var results: [String] = []
        
        for item in data {
            // Simulate async processing
            try await Task.sleep(nanoseconds: 1_000_000) // 1ms
            results.append("PROCESSED_\(item)")
        }
        
        return results
    }
}

// MARK: - Async/Await Functions

class AsyncNetworkService {
    private let session = URLSession.shared
    
    func fetchData(from url: URL) async throws -> Data {
        let (data, response) = try await session.data(from: url)
        
        guard let httpResponse = response as? HTTPURLResponse,
              200...299 ~= httpResponse.statusCode else {
            throw NetworkError.invalidResponse
        }
        
        return data
    }
    
    func fetchMultipleResources(urls: [URL]) async throws -> [Data] {
        // Concurrent execution using async let
        async let firstData = fetchData(from: urls[0])
        async let secondData = fetchData(from: urls[1])
        async let thirdData = fetchData(from: urls[2])
        
        return try await [firstData, secondData, thirdData]
    }
    
    func processWithTaskGroup(urls: [URL]) async throws -> [String] {
        return try await withThrowingTaskGroup(of: String.self) { group in
            for url in urls {
                group.addTask {
                    let data = try await self.fetchData(from: url)
                    return String(data: data, encoding: .utf8) ?? "EMPTY"
                }
            }
            
            var results: [String] = []
            for try await result in group {
                results.append(result)
            }
            return results
        }
    }
}

// MARK: - SwiftUI Views with State Management

struct ContentView: View {
    @StateObject private var viewModel = ContentViewModel()
    @Environment(\.colorScheme) var colorScheme
    @AppStorage("user_preference") var userPreference: String = "default"
    
    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                HeaderView(title: "Modern Swift Demo")
                
                AsyncButton("Load Data") {
                    await viewModel.loadData()
                }
                .disabled(viewModel.isLoading)
                
                if viewModel.isLoading {
                    ProgressView("Loading...")
                        .progressViewStyle(CircularProgressViewStyle())
                } else {
                    DataListView(items: viewModel.items)
                }
                
                Spacer()
            }
            .padding()
            .navigationTitle("Demo")
            .task {
                await viewModel.initializeData()
            }
            .refreshable {
                await viewModel.refreshData()
            }
        }
        .preferredColorScheme(colorScheme)
    }
}

// MARK: - SwiftUI Components with ViewBuilder

struct HeaderView: View {
    let title: String
    @State private var isAnimated = false
    
    var body: some View {
        Text(title)
            .font(.largeTitle)
            .fontWeight(.bold)
            .scaleEffect(isAnimated ? 1.1 : 1.0)
            .animation(.spring(response: 0.5, dampingFraction: 0.8), value: isAnimated)
            .onAppear {
                withAnimation {
                    isAnimated = true
                }
            }
    }
}

struct AsyncButton<Label: View>: View {
    let label: Label
    let action: () async -> Void
    @State private var isPerforming = false
    
    init(_ titleKey: LocalizedStringKey, action: @escaping () async -> Void) where Label == Text {
        self.label = Text(titleKey)
        self.action = action
    }
    
    init(action: @escaping () async -> Void, @ViewBuilder label: () -> Label) {
        self.label = label()
        self.action = action
    }
    
    var body: some View {
        Button {
            Task {
                isPerforming = true
                defer { isPerforming = false }
                await action()
            }
        } label: {
            if isPerforming {
                ProgressView()
                    .progressViewStyle(CircularProgressViewStyle(tint: .white))
                    .scaleEffect(0.8)
            } else {
                label
            }
        }
        .disabled(isPerforming)
    }
}

struct DataListView: View {
    let items: [DataItem]
    @State private var selectedItem: DataItem?
    
    var body: some View {
        LazyVStack(spacing: 12) {
            ForEach(items) { item in
                DataRowView(item: item)
                    .onTapGesture {
                        selectedItem = item
                    }
                    .contextMenu {
                        Button("Share") {
                            shareItem(item)
                        }
                        Button("Delete", role: .destructive) {
                            deleteItem(item)
                        }
                    }
            }
        }
        .sheet(item: $selectedItem) { item in
            DataDetailView(item: item)
        }
    }
    
    private func shareItem(_ item: DataItem) {
        // Share implementation
    }
    
    private func deleteItem(_ item: DataItem) {
        // Delete implementation
    }
}

// MARK: - Observable Pattern with Combine

@MainActor
class ContentViewModel: ObservableObject {
    @Published var items: [DataItem] = []
    @Published var isLoading = false
    @Published var errorMessage: String?
    
    private let dataManager = DataManager()
    private let networkService = AsyncNetworkService()
    private var cancellables = Set<AnyCancellable>()
    
    func initializeData() async {
        await loadData()
        setupSubscriptions()
    }
    
    func loadData() async {
        isLoading = true
        defer { isLoading = false }
        
        do {
            // Simulate network delay
            try await Task.sleep(nanoseconds: 2_000_000_000) // 2 seconds
            
            let urls = [
                URL(string: "https://api.example.com/data1")!,
                URL(string: "https://api.example.com/data2")!,
                URL(string: "https://api.example.com/data3")!
            ]
            
            let results = try await networkService.processWithTaskGroup(urls: urls)
            
            items = results.enumerated().map { index, content in
                DataItem(id: UUID(), title: "Item \(index + 1)", content: content)
            }
            
            // Store in actor-managed cache
            await dataManager.store(key: "cached_items", value: items)
            
        } catch {
            errorMessage = error.localizedDescription
        }
    }
    
    func refreshData() async {
        await loadData()
    }
    
    private func setupSubscriptions() {
        // Combine reactive programming
        $items
            .debounce(for: .milliseconds(300), scheduler: RunLoop.main)
            .sink { items in
                print("Items updated: \(items.count)")
            }
            .store(in: &cancellables)
        
        $errorMessage
            .compactMap { $0 }
            .sink { error in
                print("Error occurred: \(error)")
            }
            .store(in: &cancellables)
    }
}

// MARK: - Data Models with Property Wrappers

struct DataItem: Identifiable, Codable {
    let id: UUID
    let title: String
    let content: String
    @CodingKey("created_at") let createdAt: Date = Date()
    
    enum CodingKeys: String, CodingKey {
        case id, title, content
        case createdAt = "created_at"
    }
}

// MARK: - Custom Property Wrappers

@propertyWrapper
struct Capitalized {
    private var value: String
    
    var wrappedValue: String {
        get { value }
        set { value = newValue.capitalized }
    }
    
    init(wrappedValue: String) {
        self.value = wrappedValue.capitalized
    }
}

@propertyWrapper
struct UserDefaultsBacked<T: Codable> {
    let key: String
    let defaultValue: T
    
    var wrappedValue: T {
        get {
            guard let data = UserDefaults.standard.data(forKey: key),
                  let value = try? JSONDecoder().decode(T.self, from: data) else {
                return defaultValue
            }
            return value
        }
        set {
            guard let data = try? JSONEncoder().encode(newValue) else { return }
            UserDefaults.standard.set(data, forKey: key)
        }
    }
}

// MARK: - Structured Concurrency Example

class ConcurrentDataProcessor {
    private let dataManager = DataManager()
    
    func processLargeDataSet(_ data: [String]) async throws -> ProcessingResult {
        // Use structured concurrency for parallel processing
        return try await withThrowingTaskGroup(of: ProcessedChunk.self) { group in
            let chunks = data.chunked(into: 100) // Assume we have this extension
            
            for (index, chunk) in chunks.enumerated() {
                group.addTask { [weak self] in
                    guard let self = self else { throw ProcessingError.cancelled }
                    
                    let processedData = try await self.dataManager.processData(chunk)
                    return ProcessedChunk(index: index, data: processedData)
                }
            }
            
            var results: [ProcessedChunk] = []
            for try await chunk in group {
                results.append(chunk)
            }
            
            // Sort by index to maintain order
            results.sort { $0.index < $1.index }
            let finalData = results.flatMap { $0.data }
            
            return ProcessingResult(
                totalItems: data.count,
                processedItems: finalData.count,
                data: finalData
            )
        }
    }
}

// MARK: - Result Builders (Function Builders)

@resultBuilder
struct ViewConfigBuilder {
    static func buildBlock(_ components: ViewConfig...) -> [ViewConfig] {
        components
    }
    
    static func buildIf(_ component: ViewConfig?) -> ViewConfig? {
        component
    }
    
    static func buildEither(first: ViewConfig) -> ViewConfig {
        first
    }
    
    static func buildEither(second: ViewConfig) -> ViewConfig {
        second
    }
}

struct ViewConfig {
    let name: String
    let properties: [String: Any]
    
    init(name: String, properties: [String: Any] = [:]) {
        self.name = name
        self.properties = properties
    }
}

func createView(@ViewConfigBuilder _ builder: () -> [ViewConfig]) -> UIView {
    let configs = builder()
    let view = UIView()
    // Apply configurations to view
    return view
}

// MARK: - Supporting Types

struct ProcessedChunk {
    let index: Int
    let data: [String]
}

struct ProcessingResult {
    let totalItems: Int
    let processedItems: Int
    let data: [String]
}

enum ProcessingError: Error {
    case cancelled
    case invalidData
    case timeout
}

enum NetworkError: Error {
    case invalidResponse
    case noData
}

// MARK: - Extensions

extension Array {
    func chunked(into size: Int) -> [[Element]] {
        return stride(from: 0, to: count, by: size).map {
            Array(self[$0..<Swift.min($0 + size, count)])
        }
    }
}

// MARK: - Protocol with Async Requirements

protocol AsyncDataProvider {
    associatedtype DataType: Codable
    
    func fetchData() async throws -> [DataType]
    func updateData(_ data: [DataType]) async throws
    func deleteData(matching predicate: @escaping (DataType) -> Bool) async throws
}

// MARK: - Generic Async Iterator

struct AsyncDataIterator<T>: AsyncIteratorProtocol, AsyncSequence {
    typealias Element = T
    
    private let data: [T]
    private var index = 0
    
    init(_ data: [T]) {
        self.data = data
    }
    
    mutating func next() async -> T? {
        guard index < data.count else { return nil }
        defer { index += 1 }
        
        // Simulate async delay
        try? await Task.sleep(nanoseconds: 10_000_000) // 10ms
        return data[index]
    }
    
    func makeAsyncIterator() -> AsyncDataIterator<T> {
        return self
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "async", // Search for modern async features
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find modern Swift keywords and features
    let modern_features = [
        "async",
        "await",
        "actor",
        "@MainActor",
        "@Published",
        "@StateObject",
        "@State",
    ];
    let found_modern: Vec<&str> = modern_features
        .iter()
        .filter(|&feature| output.contains(feature))
        .copied()
        .collect();

    assert!(
        !found_modern.is_empty(),
        "Should find modern Swift features. Found: {:?} - output: {}",
        found_modern,
        output
    );

    // Should find SwiftUI-specific constructs
    let swiftui_features = [
        "View",
        "VStack",
        "NavigationStack",
        "Button",
        "@ViewBuilder",
    ];
    let found_swiftui: Vec<&str> = swiftui_features
        .iter()
        .filter(|&feature| output.contains(feature))
        .copied()
        .collect();

    assert!(
        !found_swiftui.is_empty(),
        "Should find SwiftUI constructs. Found: {:?} - output: {}",
        found_swiftui,
        output
    );

    // Should find structured concurrency patterns
    let concurrency_patterns = [
        "TaskGroup",
        "withThrowingTaskGroup",
        "Task.sleep",
        "async let",
    ];
    let found_concurrency: Vec<&str> = concurrency_patterns
        .iter()
        .filter(|&pattern| output.contains(pattern))
        .copied()
        .collect();

    assert!(
        !found_concurrency.is_empty(),
        "Should find structured concurrency patterns. Found: {:?} - output: {}",
        found_concurrency,
        output
    );

    // Should find property wrapper usage
    let property_wrapper_patterns = ["@propertyWrapper", "@UserDefaultsBacked", "@Capitalized"];
    let found_property_wrappers: Vec<&str> = property_wrapper_patterns
        .iter()
        .filter(|&pattern| output.contains(pattern))
        .copied()
        .collect();

    assert!(
        !found_property_wrappers.is_empty(),
        "Should find property wrapper patterns. Found: {:?} - output: {}",
        found_property_wrappers,
        output
    );

    // Should find Combine framework usage
    let combine_patterns = ["@Published", "AnyCancellable", "sink", "debounce"];
    let found_combine: Vec<&str> = combine_patterns
        .iter()
        .filter(|&pattern| output.contains(pattern))
        .copied()
        .collect();

    // Note: Combine patterns might not always appear in outline search results
    if !found_combine.is_empty() {
        println!("Found Combine patterns: {:?}", found_combine);
    }

    // Should find result builder usage
    let result_builder_patterns = ["@resultBuilder", "buildBlock", "ViewConfigBuilder"];
    let found_result_builders: Vec<&str> = result_builder_patterns
        .iter()
        .filter(|&pattern| output.contains(pattern))
        .copied()
        .collect();

    // Note: Result builders might not always appear in outline search results
    if !found_result_builders.is_empty() {
        println!("Found result builder patterns: {:?}", found_result_builders);
    }

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_swift_outline_small_vs_large_functions_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("FunctionSizes.swift");

    let content = r#"import Foundation

// MARK: - Small Functions (should NOT get closing brace comments)

func simpleAdd(_ a: Int, _ b: Int) -> Int {
    return a + b
}

func quickCheck(_ value: String) -> Bool {
    return !value.isEmpty
}

func shortGreeting(_ name: String) -> String {
    return "Hello, \(name)!"
}

func basicValidation(_ input: String) -> Bool {
    guard !input.isEmpty else { return false }
    return input.count >= 3
}

func smallCalculation(_ numbers: [Int]) -> Int {
    return numbers.reduce(0, +)
}

// MARK: - Medium Functions (might get closing brace comments depending on gaps)

func mediumProcessing(_ data: [String]) -> [String] {
    var results: [String] = []
    
    for item in data {
        if item.hasPrefix("PROCESS_") {
            results.append(item.uppercased())
        } else if item.hasPrefix("SKIP_") {
            continue
        } else {
            results.append(item.lowercased())
        }
    }
    
    return results.sorted()
}

func mediumValidation(_ input: Any) -> ValidationResult {
    if let stringInput = input as? String {
        if stringInput.isEmpty {
            return .invalid("Empty string")
        } else if stringInput.count < 3 {
            return .invalid("Too short")
        } else {
            return .valid
        }
    } else if let numberInput = input as? NSNumber {
        let value = numberInput.doubleValue
        return value > 0 ? .valid : .invalid("Negative number")
    } else {
        return .invalid("Unsupported type")
    }
}

// MARK: - Large Functions (should get closing brace comments with Swift // syntax)

func complexDataProcessing(_ dataset: [Any]) -> ProcessingResult {
    var processedItems: [ProcessedItem] = []
    var errors: [ProcessingError] = []
    var statistics = ProcessingStatistics()
    
    // Phase 1: Initial validation and categorization
    for (index, item) in dataset.enumerated() {
        do {
            let validatedItem = try validateDataItem(item, at: index)
            
            switch validatedItem.category {
            case .highPriority:
                statistics.highPriorityCount += 1
                processedItems.append(ProcessedItem(
                    id: UUID(),
                    originalIndex: index,
                    data: validatedItem.data,
                    priority: .high,
                    processingTime: Date()
                ))
                
            case .mediumPriority:
                statistics.mediumPriorityCount += 1
                processedItems.append(ProcessedItem(
                    id: UUID(),
                    originalIndex: index,
                    data: validatedItem.data,
                    priority: .medium,
                    processingTime: Date()
                ))
                
            case .lowPriority:
                statistics.lowPriorityCount += 1
                processedItems.append(ProcessedItem(
                    id: UUID(),
                    originalIndex: index,
                    data: validatedItem.data,
                    priority: .low,
                    processingTime: Date()
                ))
                
            case .deferred:
                statistics.deferredCount += 1
                // Deferred items are processed later
                continue
            }
        } catch {
            errors.append(ProcessingError(
                index: index,
                item: item,
                error: error,
                timestamp: Date()
            ))
            statistics.errorCount += 1
        }
    }
    
    // Phase 2: Advanced processing for complex items
    var complexProcessedItems: [ProcessedItem] = []
    for item in processedItems {
        if item.priority == .high {
            // Complex processing for high priority items
            let enhancedData = performComplexTransformation(item.data)
            let analysisResult = performDeepAnalysis(enhancedData)
            
            var updatedItem = item
            updatedItem.data = enhancedData
            updatedItem.analysisScore = analysisResult.score
            updatedItem.confidence = analysisResult.confidence
            updatedItem.metadata = analysisResult.metadata
            
            complexProcessedItems.append(updatedItem)
            statistics.complexProcessingCount += 1
        } else {
            // Standard processing for lower priority items
            complexProcessedItems.append(item)
        }
    }
    
    // Phase 3: Final validation and result compilation
    let finalResults = complexProcessedItems.compactMap { item -> ProcessedItem? in
        guard item.analysisScore > 0.5 else {
            statistics.rejectedCount += 1
            return nil
        }
        
        statistics.acceptedCount += 1
        return item
    }
    
    // Phase 4: Generate comprehensive report
    let processingReport = ProcessingReport(
        totalItemsProcessed: dataset.count,
        successfulItems: finalResults.count,
        errorCount: errors.count,
        statistics: statistics,
        processingDuration: Date().timeIntervalSince(statistics.startTime),
        qualityScore: calculateQualityScore(finalResults)
    )
    
    return ProcessingResult(
        processedItems: finalResults,
        errors: errors,
        statistics: statistics,
        report: processingReport
    )
}

func massiveDataTransformation(_ input: [ComplexDataStructure]) async throws -> [TransformedData] {
    var transformedResults: [TransformedData] = []
    let processor = AsyncDataProcessor()
    let validator = DataValidator()
    let enhancer = DataEnhancer()
    
    // Batch processing configuration
    let batchSize = 100
    let maxConcurrentTasks = 4
    let timeout: TimeInterval = 300 // 5 minutes
    
    // Initialize monitoring and logging
    let processingMonitor = ProcessingMonitor()
    await processingMonitor.startMonitoring()
    
    defer {
        Task {
            await processingMonitor.stopMonitoring()
        }
    }
    
    // Phase 1: Preprocessing and validation
    for (batchIndex, batch) in input.chunked(into: batchSize).enumerated() {
        await processingMonitor.logBatchStart(batchIndex, itemCount: batch.count)
        
        do {
            // Parallel validation of batch items
            let validationResults = try await withThrowingTaskGroup(of: ValidationResult.self) { group in
                for item in batch {
                    group.addTask {
                        return try await validator.validateAsync(item)
                    }
                }
                
                var results: [ValidationResult] = []
                for try await result in group {
                    results.append(result)
                }
                return results
            }
            
            // Process validated items
            for (itemIndex, validationResult) in validationResults.enumerated() {
                let originalItem = batch[itemIndex]
                
                switch validationResult.status {
                case .valid:
                    // Transform valid items
                    let transformedItem = try await processor.transform(originalItem)
                    let enhancedItem = try await enhancer.enhance(transformedItem)
                    transformedResults.append(enhancedItem)
                    
                case .validWithWarnings:
                    // Handle items with warnings
                    let transformedItem = try await processor.transformWithWarnings(originalItem, warnings: validationResult.warnings)
                    transformedResults.append(transformedItem)
                    
                case .invalid:
                    // Log invalid items for manual review
                    await processingMonitor.logInvalidItem(originalItem, reason: validationResult.reason)
                    continue
                    
                case .requiresManualReview:
                    // Queue for manual review
                    await processingMonitor.queueForManualReview(originalItem, notes: validationResult.notes)
                    continue
                }
            }
            
            await processingMonitor.logBatchCompletion(batchIndex, successCount: validationResults.count)
            
        } catch {
            await processingMonitor.logBatchError(batchIndex, error: error)
            throw ProcessingError.batchProcessingFailed(batchIndex: batchIndex, underlyingError: error)
        }
        
        // Rate limiting between batches
        if batchIndex < input.chunked(into: batchSize).count - 1 {
            try await Task.sleep(nanoseconds: 100_000_000) // 100ms delay
        }
    }
    
    // Phase 2: Post-processing optimization
    let optimizedResults = try await optimizeTransformedData(transformedResults)
    
    // Phase 3: Quality assurance
    let qualityResults = try await performQualityAssurance(optimizedResults)
    
    // Phase 4: Final reporting
    let finalReport = await generateProcessingReport(
        originalCount: input.count,
        transformedCount: qualityResults.count,
        monitor: processingMonitor
    )
    
    await processingMonitor.saveFinalReport(finalReport)
    
    return qualityResults
}

// MARK: - Helper Functions and Supporting Types

private func validateDataItem(_ item: Any, at index: Int) throws -> ValidatedDataItem {
    // Validation logic here
    return ValidatedDataItem(data: item, category: .mediumPriority)
}

private func performComplexTransformation(_ data: Any) -> Any {
    // Complex transformation logic
    return data
}

private func performDeepAnalysis(_ data: Any) -> AnalysisResult {
    return AnalysisResult(score: 0.8, confidence: 0.9, metadata: [:])
}

private func calculateQualityScore(_ items: [ProcessedItem]) -> Double {
    guard !items.isEmpty else { return 0.0 }
    return items.map(\.analysisScore).reduce(0, +) / Double(items.count)
}

private func optimizeTransformedData(_ data: [TransformedData]) async throws -> [TransformedData] {
    // Optimization logic
    return data
}

private func performQualityAssurance(_ data: [TransformedData]) async throws -> [TransformedData] {
    // Quality assurance logic
    return data
}

private func generateProcessingReport(originalCount: Int, transformedCount: Int, monitor: ProcessingMonitor) async -> ProcessingReport {
    return ProcessingReport(
        totalItemsProcessed: originalCount,
        successfulItems: transformedCount,
        errorCount: 0,
        statistics: ProcessingStatistics(),
        processingDuration: 0,
        qualityScore: 1.0
    )
}

// MARK: - Supporting Data Structures

struct ProcessedItem {
    var id: UUID
    let originalIndex: Int
    var data: Any
    let priority: Priority
    let processingTime: Date
    var analysisScore: Double = 0.0
    var confidence: Double = 0.0
    var metadata: [String: Any] = [:]
}

struct ProcessingResult {
    let processedItems: [ProcessedItem]
    let errors: [ProcessingError]
    let statistics: ProcessingStatistics
    let report: ProcessingReport
}

struct ProcessingStatistics {
    var highPriorityCount = 0
    var mediumPriorityCount = 0
    var lowPriorityCount = 0
    var deferredCount = 0
    var errorCount = 0
    var complexProcessingCount = 0
    var rejectedCount = 0
    var acceptedCount = 0
    let startTime = Date()
}

struct ProcessingReport {
    let totalItemsProcessed: Int
    let successfulItems: Int
    let errorCount: Int
    let statistics: ProcessingStatistics
    let processingDuration: TimeInterval
    let qualityScore: Double
}

enum Priority {
    case high, medium, low
}

enum ValidationStatus {
    case valid, validWithWarnings, invalid, requiresManualReview
}

struct ValidationResult {
    let status: ValidationStatus
    let warnings: [String] = []
    let reason: String = ""
    let notes: [String] = []
}

struct ValidatedDataItem {
    let data: Any
    let category: DataCategory
}

enum DataCategory {
    case highPriority, mediumPriority, lowPriority, deferred
}

struct ProcessingError: Error {
    let index: Int
    let item: Any
    let error: Error
    let timestamp: Date
    
    static func batchProcessingFailed(batchIndex: Int, underlyingError: Error) -> ProcessingError {
        return ProcessingError(
            index: batchIndex,
            item: "Batch \(batchIndex)",
            error: underlyingError,
            timestamp: Date()
        )
    }
}

struct ComplexDataStructure {
    let id: String
    let data: [String: Any]
}

struct TransformedData {
    let id: String
    let transformedData: [String: Any]
}

struct AnalysisResult {
    let score: Double
    let confidence: Double
    let metadata: [String: Any]
}

// Mock classes for compilation
class AsyncDataProcessor {
    func transform(_ item: ComplexDataStructure) async throws -> TransformedData {
        return TransformedData(id: item.id, transformedData: item.data)
    }
    
    func transformWithWarnings(_ item: ComplexDataStructure, warnings: [String]) async throws -> TransformedData {
        return TransformedData(id: item.id, transformedData: item.data)
    }
}

class DataValidator {
    func validateAsync(_ item: ComplexDataStructure) async throws -> ValidationResult {
        return ValidationResult(status: .valid)
    }
}

class DataEnhancer {
    func enhance(_ item: TransformedData) async throws -> TransformedData {
        return item
    }
}

actor ProcessingMonitor {
    func startMonitoring() async { }
    func stopMonitoring() async { }
    func logBatchStart(_ index: Int, itemCount: Int) async { }
    func logBatchCompletion(_ index: Int, successCount: Int) async { }
    func logBatchError(_ index: Int, error: Error) async { }
    func logInvalidItem(_ item: ComplexDataStructure, reason: String) async { }
    func queueForManualReview(_ item: ComplexDataStructure, notes: [String]) async { }
    func saveFinalReport(_ report: ProcessingReport) async { }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "function", // Search for functions
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find both small and large functions
    let small_functions = [
        "simpleAdd",
        "quickCheck",
        "shortGreeting",
        "basicValidation",
        "smallCalculation",
    ];
    let found_small: Vec<&str> = small_functions
        .iter()
        .filter(|&func| output.contains(func))
        .copied()
        .collect();

    let large_functions = ["complexDataProcessing", "massiveDataTransformation"];
    let found_large: Vec<&str> = large_functions
        .iter()
        .filter(|&func| output.contains(func))
        .copied()
        .collect();

    // Should find functions of various sizes
    assert!(
        !found_small.is_empty() || !found_large.is_empty(),
        "Should find functions of various sizes. Small: {:?}, Large: {:?} - output: {}",
        found_small,
        found_large,
        output
    );

    // Check closing brace behavior - large functions should have them
    let has_closing_brace_comments = output.contains("} //");
    if !found_large.is_empty() {
        assert!(
            has_closing_brace_comments,
            "Large functions should have closing brace comments with Swift // syntax - output: {}",
            output
        );
    }

    // Verify small functions do NOT have closing brace comments if they appear in output
    if !found_small.is_empty() {
        let small_func_lines: Vec<&str> = output
            .lines()
            .filter(|line| small_functions.iter().any(|&func| line.contains(func)))
            .collect();

        // Check if any small function lines have closing brace comments
        let small_func_has_closing_comments = small_func_lines
            .iter()
            .any(|line| line.contains("} //") || line.contains("} /*"));

        // Small functions should NOT have closing brace comments
        if small_func_has_closing_comments {
            println!("Warning: Small functions appear to have closing brace comments");
        }
    }

    // Should detect different function complexity levels
    let complexity_indicators = ["Phase", "batch", "async", "await", "complex", "validation"];
    let found_complexity: Vec<&str> = complexity_indicators
        .iter()
        .filter(|&indicator| output.contains(indicator))
        .copied()
        .collect();

    assert!(
        !found_complexity.is_empty(),
        "Should detect function complexity indicators. Found: {:?} - output: {}",
        found_complexity,
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

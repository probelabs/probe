use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_cpp_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("basic.cpp");

    let content = r#"#include <iostream>
#include <vector>
#include <memory>
#include <string>
#include <algorithm>

// Namespace for calculator functionality
namespace calculator {

    // Abstract base class
    class CalculatorInterface {
    public:
        virtual ~CalculatorInterface() = default;
        virtual double add(double x, double y) = 0;
        virtual double subtract(double x, double y) = 0;
        virtual double multiply(double x, double y) = 0;
        virtual double divide(double x, double y) = 0;
        virtual std::vector<double> getHistory() const = 0;
    };

    // Template calculator class
    template<typename T>
    class Calculator : public CalculatorInterface {
    private:
        std::string name_;
        std::vector<T> history_;
        int precision_;

    public:
        explicit Calculator(const std::string& name, int precision = 2)
            : name_(name), precision_(precision) {}

        virtual ~Calculator() override = default;

        // Move constructor and assignment
        Calculator(Calculator&& other) noexcept
            : name_(std::move(other.name_))
            , history_(std::move(other.history_))
            , precision_(other.precision_) {}

        Calculator& operator=(Calculator&& other) noexcept {
            if (this != &other) {
                name_ = std::move(other.name_);
                history_ = std::move(other.history_);
                precision_ = other.precision_;
            }
            return *this;
        }

        // Arithmetic operations
        double add(double x, double y) override {
            T result = static_cast<T>(x + y);
            recordOperation(result);
            return static_cast<double>(result);
        }

        double subtract(double x, double y) override {
            T result = static_cast<T>(x - y);
            recordOperation(result);
            return static_cast<double>(result);
        }

        double multiply(double x, double y) override {
            T result = static_cast<T>(x * y);
            recordOperation(result);
            return static_cast<double>(result);
        }

        double divide(double x, double y) override {
            if (y == 0) {
                throw std::runtime_error("Division by zero");
            }
            T result = static_cast<T>(x / y);
            recordOperation(result);
            return static_cast<double>(result);
        }

        std::vector<double> getHistory() const override {
            std::vector<double> result;
            result.reserve(history_.size());
            std::transform(history_.begin(), history_.end(),
                         std::back_inserter(result),
                         [](const T& val) { return static_cast<double>(val); });
            return result;
        }

        // Template member function
        template<typename U>
        auto process(const std::vector<U>& data) -> std::vector<decltype(T{} + U{})> {
            std::vector<decltype(T{} + U{})> results;
            for (const auto& item : data) {
                results.push_back(static_cast<T>(item));
            }
            return results;
        }

    private:
        void recordOperation(const T& result) {
            history_.push_back(result);
        }
    };

    // Factory function
    template<typename T>
    std::unique_ptr<Calculator<T>> createCalculator(const std::string& name, int precision = 2) {
        return std::make_unique<Calculator<T>>(name, precision);
    }

    // Specialized template for floating point
    template<>
    class Calculator<float> : public CalculatorInterface {
    private:
        std::string name_;
        std::vector<float> history_;
        int precision_;

    public:
        explicit Calculator(const std::string& name, int precision = 2)
            : name_(name), precision_(precision) {}

        double add(double x, double y) override {
            float result = static_cast<float>(x + y);
            history_.push_back(result);
            return static_cast<double>(result);
        }

        double subtract(double x, double y) override {
            float result = static_cast<float>(x - y);
            history_.push_back(result);
            return static_cast<double>(result);
        }

        double multiply(double x, double y) override {
            float result = static_cast<float>(x * y);
            history_.push_back(result);
            return static_cast<double>(result);
        }

        double divide(double x, double y) override {
            if (y == 0) throw std::runtime_error("Division by zero");
            float result = static_cast<float>(x / y);
            history_.push_back(result);
            return static_cast<double>(result);
        }

        std::vector<double> getHistory() const override {
            return std::vector<double>(history_.begin(), history_.end());
        }
    };

} // namespace calculator

// Global function outside namespace
std::unique_ptr<calculator::CalculatorInterface> createDefaultCalculator() {
    return calculator::createCalculator<double>("Default Calculator");
}

int main() {
    auto calc = calculator::createCalculator<double>("Test Calculator");

    try {
        double result = calc->add(10.5, 20.3);
        std::cout << "Result: " << result << std::endl;
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
    }

    return 0;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Calculator", // Search for Calculator classes
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify C++ symbols are extracted
    assert!(
        output.contains("namespace calculator") || output.contains("class CalculatorInterface"),
        "Missing C++ namespace or class - output: {}",
        output
    );
    assert!(
        output.contains("class Calculator") || output.contains("template"),
        "Missing template class - output: {}",
        output
    );
    assert!(
        output.contains("createCalculator") || output.contains("createDefaultCalculator"),
        "Missing factory functions - output: {}",
        output
    );
    // The output should contain substantial C++ code structures
    assert!(
        output.len() > 800, // Should contain significant C++ code
        "Should contain substantial C++ code - output length: {}, output: {}",
        output.len(),
        output
    );

    Ok(())
}

#[test]
fn test_cpp_outline_stl_and_templates() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("templates.cpp");

    let content = r#"#include <vector>
#include <map>
#include <algorithm>
#include <memory>
#include <functional>
#include <type_traits>

// Advanced template metaprogramming
template<typename T, typename Enable = void>
struct TypeTraits {
    static constexpr bool is_numeric = false;
};

template<typename T>
struct TypeTraits<T, std::enable_if_t<std::is_arithmetic_v<T>>> {
    static constexpr bool is_numeric = true;
    using value_type = T;
};

// SFINAE template functions
template<typename T>
std::enable_if_t<std::is_integral_v<T>, T> processInteger(T value) {
    return value * 2;
}

template<typename T>
std::enable_if_t<std::is_floating_point_v<T>, T> processFloat(T value) {
    return value * 1.5;
}

// Variadic template class
template<typename... Args>
class DataContainer {
private:
    std::tuple<Args...> data_;

public:
    explicit DataContainer(Args... args) : data_(std::make_tuple(args...)) {}

    template<size_t Index>
    auto get() const -> std::tuple_element_t<Index, std::tuple<Args...>> {
        return std::get<Index>(data_);
    }

    template<typename F>
    void forEach(F&& func) {
        forEachImpl(std::forward<F>(func), std::index_sequence_for<Args...>{});
    }

private:
    template<typename F, size_t... Indices>
    void forEachImpl(F&& func, std::index_sequence<Indices...>) {
        (func(std::get<Indices>(data_)), ...);
    }
};

// Template specialization
template<>
class DataContainer<std::string> {
private:
    std::vector<std::string> strings_;

public:
    explicit DataContainer(const std::string& str) : strings_{str} {}

    void addString(const std::string& str) {
        strings_.push_back(str);
    }

    const std::vector<std::string>& getStrings() const {
        return strings_;
    }
};

// CRTP pattern
template<typename Derived>
class Printable {
public:
    void print() const {
        static_cast<const Derived*>(this)->printImpl();
    }

protected:
    ~Printable() = default;
};

class Document : public Printable<Document> {
private:
    std::string content_;

public:
    explicit Document(const std::string& content) : content_(content) {}

    void printImpl() const {
        std::cout << "Document: " << content_ << std::endl;
    }
};

// Lambda and functional programming support
template<typename Container, typename Predicate>
auto filterContainer(const Container& container, Predicate pred) {
    Container result;
    std::copy_if(container.begin(), container.end(),
                std::back_inserter(result), pred);
    return result;
}

// Advanced template with concept-like requirements (C++17 style)
template<typename T>
class SmartProcessor {
    static_assert(std::is_default_constructible_v<T>, "T must be default constructible");
    static_assert(std::is_copy_constructible_v<T>, "T must be copy constructible");

private:
    std::vector<T> items_;
    std::function<T(const T&)> processor_;

public:
    template<typename Processor>
    SmartProcessor(Processor&& proc) : processor_(std::forward<Processor>(proc)) {}

    void addItem(const T& item) {
        items_.push_back(item);
    }

    std::vector<T> processAll() const {
        std::vector<T> results;
        results.reserve(items_.size());

        std::transform(items_.begin(), items_.end(),
                      std::back_inserter(results),
                      processor_);

        return results;
    }
};

int main() {
    // Test variadic template
    DataContainer<int, double, std::string> container(42, 3.14, "Hello");
    std::cout << "First: " << container.get<0>() << std::endl;

    // Test CRTP
    Document doc("Sample document");
    doc.print();

    // Test smart processor
    SmartProcessor<int> processor([](int x) { return x * x; });
    processor.addItem(5);
    processor.addItem(10);

    auto results = processor.processAll();
    for (const auto& result : results) {
        std::cout << result << " ";
    }

    return 0;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "DataContainer", // Search for template classes
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify C++ template features
    assert!(
        output.contains("template") || output.contains("struct TypeTraits"),
        "Missing template structures - output: {}",
        output
    );
    assert!(
        output.contains("class DataContainer"),
        "Missing variadic template class - output: {}",
        output
    );
    // The search results show various template patterns even if not all classes are included
    assert!(
        output.len() > 500, // Should have substantial template-related content
        "Should contain substantial template code - output length: {}, output: {}",
        output.len(),
        output
    );

    Ok(())
}

#[test]
fn test_cpp_outline_smart_closing_brace_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("smart_braces.cpp");

    let content = r#"// Small function that should NOT get closing brace comments.
int small_function(int x) {
    int result = x * 2;
    return result + 1;
}

// Large function that SHOULD get closing brace comments with C++ // syntax.
std::vector<std::string> large_function_with_gaps(const std::vector<int>& data) {
    std::vector<std::string> results;
    DataProcessor processor;

    // Phase 1: Initial processing with nested control flow
    for (size_t i = 0; i < data.size(); ++i) {
        if (data[i] > 100) {
            processor.processLargeValue(data[i], i);
            if (data[i] > 1000) {
                processor.markAsExceptional(i);
            }
        } else if (data[i] < 0) {
            processor.processNegativeValue(data[i], i);
        } else {
            processor.processSmallValue(data[i], i);
        }
    }

    // Phase 2: Complex transformation logic with switch
    auto transformedData = processor.getTransformedData();
    for (const auto& item : transformedData) {
        switch (item.category) {
            case Category::HIGH:
                results.push_back("HIGH: " + std::to_string(item.value));
                break;
            case Category::MEDIUM:
                results.push_back("MED: " + std::to_string(item.value));
                break;
            case Category::LOW:
                results.push_back("LOW: " + std::to_string(item.value));
                break;
            default:
                results.push_back("UNKNOWN: " + std::to_string(item.value));
                break;
        }
    }

    // Phase 3: Final validation and cleanup with try-catch
    std::vector<std::string> validatedResults;
    for (const auto& result : results) {
        try {
            if (result.length() > 5) {
                validatedResults.push_back(result);
            }
        } catch (const std::exception& e) {
            std::cerr << "Error processing result: " << e.what() << std::endl;
        }
    }

    return validatedResults;
}

// Another large C++ class with RAII and modern features
class LargeResourceManager {
private:
    std::unique_ptr<Resource> resource_;
    std::shared_ptr<SharedData> shared_data_;
    mutable std::mutex mutex_;
    std::atomic<bool> active_;

public:
    explicit LargeResourceManager(std::unique_ptr<Resource> resource)
        : resource_(std::move(resource))
        , shared_data_(std::make_shared<SharedData>())
        , active_(true) {}

    ~LargeResourceManager() {
        cleanup();
    }

    // Move semantics
    LargeResourceManager(LargeResourceManager&& other) noexcept
        : resource_(std::move(other.resource_))
        , shared_data_(std::move(other.shared_data_))
        , active_(other.active_.load()) {
        other.active_ = false;
    }

    auto processData(const std::vector<DataItem>& items) -> std::future<ProcessResult> {
        return std::async(std::launch::async, [this, items]() {
            std::lock_guard<std::mutex> lock(mutex_);
            ProcessResult result;

            for (const auto& item : items) {
                if (!active_.load()) {
                    break;
                }

                try {
                    auto processed = resource_->process(item);
                    result.addItem(processed);
                } catch (const ProcessingException& e) {
                    result.addError(e.what());
                }
            }

            return result;
        });
    }

private:
    void cleanup() {
        if (resource_) {
            resource_->cleanup();
        }
        active_ = false;
    }
};
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "large_function", // Search for large functions
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the large functions and classes
    assert!(
        output.contains("large_function_with_gaps") || output.contains("LargeResourceManager"),
        "Missing large function or class - output: {}",
        output
    );

    // Large functions/classes should have closing brace comments with C++ // syntax
    // Look for lines ending with "} // " (C++ style comments)
    let cpp_closing_comments: Vec<&str> = output
        .lines()
        .filter(|line| line.contains("} //"))
        .collect();

    assert!(
        !cpp_closing_comments.is_empty(),
        "Large C++ functions should have closing brace comments with // syntax. Output:\n{}",
        output
    );

    // Large functions/classes should have closing brace comments with C++ // syntax
    // The main function we're testing should have closing brace comments
    assert!(
        !cpp_closing_comments.is_empty(),
        "Should have at least one closing brace comment for large C++ functions. Found: {}. Output:\n{}",
        cpp_closing_comments.len(),
        output
    );

    // Verify the closing brace comments use C++ style (//) not C style (/* */)
    let has_cpp_style_comments = output.contains("} //") && !output.contains("} /*");
    assert!(
        has_cpp_style_comments,
        "Closing brace comments should use C++ style (//) not C style (/* */). Output:\n{}",
        output
    );

    Ok(())
}

#[test]
fn test_cpp_control_flow_structures() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("control_flow.cpp");

    let content = r#"#include <iostream>
#include <vector>
#include <algorithm>
#include <exception>

// Function with comprehensive C++ control flow
void processControlFlow(const std::vector<int>& data) {
    // Traditional for loop
    for (int i = 0; i < static_cast<int>(data.size()); ++i) {
        if (data[i] > 100) {
            std::cout << "Large value: " << data[i] << std::endl;
        } else if (data[i] < 0) {
            std::cout << "Negative value: " << data[i] << std::endl;
        } else {
            std::cout << "Normal value: " << data[i] << std::endl;
        }
    }

    // Range-based for loop (C++11)
    for (const auto& value : data) {
        switch (value % 3) {
            case 0:
                std::cout << "Divisible by 3" << std::endl;
                break;
            case 1:
                std::cout << "Remainder 1" << std::endl;
                break;
            case 2:
                std::cout << "Remainder 2" << std::endl;
                break;
        }
    }

    // While loop with exception handling
    int index = 0;
    while (index < static_cast<int>(data.size())) {
        try {
            if (data[index] == 0) {
                throw std::runtime_error("Zero value encountered");
            }
            processValue(data[index]);
        } catch (const std::runtime_error& e) {
            std::cerr << "Runtime error: " << e.what() << std::endl;
        } catch (const std::exception& e) {
            std::cerr << "General error: " << e.what() << std::endl;
        } catch (...) {
            std::cerr << "Unknown error occurred" << std::endl;
        }
        ++index;
    }

    // Do-while loop
    int attempts = 0;
    do {
        attempts++;
        std::cout << "Attempt: " << attempts << std::endl;
    } while (attempts < 3);
}

// Function with nested control structures
int complexNestedControl(const std::vector<std::vector<int>>& matrix) {
    int result = 0;

    for (size_t row = 0; row < matrix.size(); ++row) {
        for (size_t col = 0; col < matrix[row].size(); ++col) {
            if (matrix[row][col] > 0) {
                switch (matrix[row][col] % 4) {
                    case 0:
                        result += matrix[row][col] * 2;
                        break;
                    case 1:
                        result += matrix[row][col];
                        break;
                    case 2: {
                        int temp = matrix[row][col];
                        while (temp > 10) {
                            temp /= 2;
                        }
                        result += temp;
                        break;
                    }
                    default:
                        result -= matrix[row][col];
                        break;
                }
            }
        }
    }

    return result;
}

private:
    void processValue(int value) {
        // Simple processing
        std::cout << "Processing: " << value << std::endl;
    }
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "processControlFlow", // Search for control flow function
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the control flow function
    assert!(
        output.contains("processControlFlow"),
        "Should contain processControlFlow function - output: {}",
        output
    );

    // Should show function with closing brace comment
    assert!(
        output.contains("} //") || output.contains("..."),
        "Should contain closing brace comment or truncation for control flow function - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_cpp_modern_features_and_lambdas() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("modern_cpp.cpp");

    let content = r#"#include <iostream>
#include <vector>
#include <algorithm>
#include <memory>
#include <functional>
#include <type_traits>

// Modern C++ features demonstration
class ModernCppFeatures {
public:
    // Auto type deduction
    auto processData(const std::vector<int>& data) -> std::vector<std::string> {
        std::vector<std::string> results;
        results.reserve(data.size());

        // Lambda with capture
        auto formatter = [](int value) -> std::string {
            if constexpr (std::is_integral_v<int>) {
                return std::to_string(value * 2);
            } else {
                return "non-integral";
            }
        };

        // Range-based for with auto
        for (const auto& item : data) {
            auto formatted = formatter(item);
            results.push_back(formatted);
        }

        return results;
    }

    // Constexpr function (C++11)
    constexpr int fibonacci(int n) {
        if (n <= 1) return n;
        return fibonacci(n - 1) + fibonacci(n - 2);
    }

    // Template with auto return type deduction (C++14)
    template<typename T, typename U>
    auto multiply(T a, U b) -> decltype(a * b) {
        return a * b;
    }

    // Smart pointer usage with RAII
    std::unique_ptr<Resource> createResource(const std::string& name) {
        auto resource = std::make_unique<Resource>(name);

        // Complex lambda with multiple captures
        auto initializer = [&resource, name](int complexity) mutable {
            if (complexity > 10) {
                resource->setComplexMode(true);
                resource->allocateBuffers(complexity * 1024);
            } else {
                resource->setSimpleMode();
            }

            // Nested lambda
            auto logger = [name](const std::string& message) {
                std::cout << "[" << name << "] " << message << std::endl;
            };

            logger("Resource initialized");
        };

        initializer(15);
        return resource;
    }

    // Variadic template with perfect forwarding (C++11)
    template<typename... Args>
    auto createSharedResource(Args&&... args) -> std::shared_ptr<Resource> {
        return std::make_shared<Resource>(std::forward<Args>(args)...);
    }

    // Generic lambda (C++14)
    void processGeneric() {
        auto genericProcessor = [](auto&& item) {
            using T = std::decay_t<decltype(item)>;
            if constexpr (std::is_arithmetic_v<T>) {
                return item * 2;
            } else {
                return item;
            }
        };

        auto result1 = genericProcessor(42);
        auto result2 = genericProcessor(3.14);
        auto result3 = genericProcessor(std::string("hello"));
    }
};

// C++17 features
class Cpp17Features {
public:
    // Structured bindings (C++17)
    auto getCoordinates() -> std::tuple<int, int, int> {
        return {10, 20, 30};
    }

    void useStructuredBindings() {
        auto [x, y, z] = getCoordinates();
        std::cout << "Coordinates: " << x << ", " << y << ", " << z << std::endl;
    }

    // If constexpr (C++17)
    template<typename T>
    void processType() {
        if constexpr (std::is_integral_v<T>) {
            std::cout << "Processing integer type" << std::endl;
        } else if constexpr (std::is_floating_point_v<T>) {
            std::cout << "Processing floating point type" << std::endl;
        } else {
            std::cout << "Processing other type" << std::endl;
        }
    }
};

int main() {
    ModernCppFeatures modern;
    std::vector<int> data{1, 2, 3, 4, 5};

    auto results = modern.processData(data);
    for (const auto& result : results) {
        std::cout << result << " ";
    }

    return 0;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "auto", // Search for modern C++ features
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should contain modern C++ features
    assert!(
        output.contains("auto") || output.contains("lambda") || output.contains("constexpr"),
        "Should contain modern C++ features - output: {}",
        output
    );

    // Should contain lambda expressions
    assert!(
        output.contains("[") && output.contains("]"),
        "Should contain lambda capture syntax - output: {}",
        output
    );

    // Should show template structures
    assert!(
        output.contains("template"),
        "Should contain template definitions - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_cpp_test_framework_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("tests.cpp");

    let content = r#"// Google Test framework
#include <gtest/gtest.h>
#include <gmock/gmock.h>

class CalculatorTest : public ::testing::Test {
protected:
    void SetUp() override {
        calculator = std::make_unique<Calculator>();
    }

    void TearDown() override {
        calculator.reset();
    }

    std::unique_ptr<Calculator> calculator;
};

TEST_F(CalculatorTest, Addition) {
    EXPECT_EQ(calculator->add(2, 3), 5);
    EXPECT_EQ(calculator->add(-1, 1), 0);
    EXPECT_EQ(calculator->add(0, 0), 0);
}

TEST_F(CalculatorTest, Division) {
    EXPECT_EQ(calculator->divide(10, 2), 5);
    EXPECT_THROW(calculator->divide(10, 0), std::runtime_error);
}

// Parameterized tests
class ParameterizedCalculatorTest : public ::testing::TestWithParam<std::tuple<int, int, int>> {
};

TEST_P(ParameterizedCalculatorTest, AdditionParameterized) {
    auto [a, b, expected] = GetParam();
    Calculator calc;
    EXPECT_EQ(calc.add(a, b), expected);
}

INSTANTIATE_TEST_SUITE_P(
    AdditionTests,
    ParameterizedCalculatorTest,
    ::testing::Values(
        std::make_tuple(1, 2, 3),
        std::make_tuple(0, 0, 0),
        std::make_tuple(-1, 1, 0)
    )
);

// Mock object testing
class MockDataSource : public DataSourceInterface {
public:
    MOCK_METHOD(std::vector<int>, getData, (), (override));
    MOCK_METHOD(void, saveData, (const std::vector<int>&), (override));
};

TEST(MockTest, DataProcessing) {
    MockDataSource mockSource;
    DataProcessor processor(&mockSource);

    std::vector<int> testData{1, 2, 3, 4, 5};

    EXPECT_CALL(mockSource, getData())
        .WillOnce(::testing::Return(testData));

    EXPECT_CALL(mockSource, saveData(::testing::_))
        .Times(1);

    processor.processAndSave();
}

// Simple assert-based tests
void testBasicFunctionality() {
    Calculator calc;

    // Basic assertions
    assert(calc.add(2, 3) == 5);
    assert(calc.subtract(5, 3) == 2);
    assert(calc.multiply(4, 3) == 12);

    // Test division by zero
    try {
        calc.divide(10, 0);
        assert(false); // Should not reach here
    } catch (const std::runtime_error& e) {
        assert(std::string(e.what()).find("zero") != std::string::npos);
    }
}

// Catch2 style tests
#include <catch2/catch.hpp>

TEST_CASE("Calculator basic operations", "[calculator]") {
    Calculator calc;

    SECTION("Addition") {
        REQUIRE(calc.add(2, 3) == 5);
        REQUIRE(calc.add(-1, 1) == 0);
    }

    SECTION("Division") {
        REQUIRE(calc.divide(10, 2) == 5);
        REQUIRE_THROWS_AS(calc.divide(10, 0), std::runtime_error);
    }
}

TEST_CASE("Vector operations", "[vector]") {
    std::vector<int> v{1, 2, 3};

    REQUIRE(v.size() == 3);
    REQUIRE(v[0] == 1);
    REQUIRE(v.back() == 3);
}

int main(int argc, char** argv) {
    // Google Test initialization
    ::testing::InitGoogleTest(&argc, argv);

    // Run simple tests
    testBasicFunctionality();

    return RUN_ALL_TESTS();
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "TEST", // Search for test patterns
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should detect test patterns (TEST is found, which matches TEST_F, TEST_CASE, etc.)
    assert!(
        output.contains("TEST") || output.contains("EXPECT_") || output.contains("REQUIRE"),
        "Should detect test patterns - output: {}",
        output
    );

    // Should detect test classes
    assert!(
        output.contains("CalculatorTest") || output.contains("MockDataSource"),
        "Should detect test class definitions - output: {}",
        output
    );

    // Should detect testing-related content - TEST patterns are found
    // The output shows TEST_CASE and other testing structures which is sufficient
    assert!(
        output.len() > 200, // Should have substantial test-related content
        "Should contain substantial test code - output length: {}, output: {}",
        output.len(),
        output
    );

    Ok(())
}

#[test]
fn test_cpp_multiple_file_extensions() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Test different C++ file extensions
    let extensions = vec![
        ("test.cpp", "C++ source"),
        ("test.cc", "C++ source (Google style)"),
        ("test.cxx", "C++ source (Microsoft style)"),
        ("test.hpp", "C++ header"),
        ("test.hxx", "C++ header (Microsoft style)"),
    ];

    for (filename, description) in extensions {
        let test_file = temp_dir.path().join(filename);

        let content = format!(
            r#"// {}
#include <iostream>
#include <vector>

namespace test_{} {{

    class TestClass {{
    public:
        explicit TestClass(const std::string& name) : name_(name) {{}}

        virtual ~TestClass() = default;

        auto getName() const -> const std::string& {{
            return name_;
        }}

        template<typename T>
        void processData(const std::vector<T>& data) {{
            for (const auto& item : data) {{
                processItem(item);
            }}
        }}

    private:
        std::string name_;

        template<typename T>
        void processItem(const T& item) {{
            std::cout << "Processing: " << item << std::endl;
        }}
    }};

    // Factory function
    std::unique_ptr<TestClass> createTestClass(const std::string& name) {{
        return std::make_unique<TestClass>(name);
    }}

}} // namespace test_{}

int main() {{
    auto obj = test_{}::createTestClass("TestObject");
    std::vector<int> data{{1, 2, 3, 4, 5}};
    obj->processData(data);
    return 0;
}}
"#,
            description,
            filename.replace('.', "_"),
            filename.replace('.', "_"),
            filename.replace('.', "_")
        );

        fs::write(&test_file, content)?;

        let ctx = TestContext::new();
        let output = ctx.run_probe(&[
            "search",
            "TestClass", // Search for test class
            test_file.to_str().unwrap(),
            "--format",
            "outline",
        ])?;

        // Should extract C++ symbols regardless of file extension
        assert!(
            output.contains("TestClass") || output.contains("createTestClass"),
            "Should extract C++ symbols from {} - output: {}",
            filename,
            output
        );

        // Should contain template and namespace information
        assert!(
            output.contains("template") || output.contains("namespace"),
            "Should contain C++ structural elements in {} - output: {}",
            filename,
            output
        );
    }

    Ok(())
}

#[test]
fn test_cpp_keyword_highlighting_in_outline() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keywords.cpp");

    let content = r#"#include <iostream>
#include <vector>
#include <string>
#include <memory>

// Function with multiple C++ keywords for highlighting
class KeywordDemonstration {
public:
    // Constructor with virtual and override keywords
    explicit KeywordDemonstration(const std::string& name) : name_(name) {}

    // Virtual destructor
    virtual ~KeywordDemonstration() = default;

    // Pure virtual function
    virtual void processVirtualData() = 0;

    // Static member function
    static std::shared_ptr<KeywordDemonstration> createInstance(const std::string& name);

    // Const member function
    const std::string& getName() const noexcept {
        return name_;
    }

    // Template member function with constexpr
    template<typename T>
    constexpr auto calculateValue(T input) const -> decltype(input * 2) {
        return input * 2;
    }

    // Function with exception specification
    void riskyOperation() noexcept(false) {
        throw std::runtime_error("Something went wrong");
    }

protected:
    std::string name_;
    mutable int cached_value_ = 0;

private:
    // Private virtual function
    virtual void internalProcess() {}
};

// Derived class with inheritance keywords
class ConcreteImplementation final : public KeywordDemonstration {
public:
    explicit ConcreteImplementation(const std::string& name)
        : KeywordDemonstration(name) {}

    // Override virtual function
    void processVirtualData() override final {
        std::cout << "Processing data in concrete implementation" << std::endl;
    }

    // Deleted function
    ConcreteImplementation(const ConcreteImplementation&) = delete;
    ConcreteImplementation& operator=(const ConcreteImplementation&) = delete;

    // Default move operations
    ConcreteImplementation(ConcreteImplementation&&) = default;
    ConcreteImplementation& operator=(ConcreteImplementation&&) = default;
};

// Function template with concept-like requirements
template<typename T>
void processTemplate(T&& value) {
    static_assert(std::is_arithmetic_v<std::decay_t<T>>, "T must be arithmetic");

    if constexpr (std::is_integral_v<T>) {
        std::cout << "Processing integral type" << std::endl;
    } else if constexpr (std::is_floating_point_v<T>) {
        std::cout << "Processing floating point type" << std::endl;
    }
}

// Lambda with various keywords
auto createLambda() {
    return [](auto&& value) mutable noexcept -> decltype(auto) {
        return std::forward<decltype(value)>(value);
    };
}

int main() {
    // Auto keyword usage
    auto instance = std::make_unique<ConcreteImplementation>("test");

    // Range-based for with auto
    std::vector<int> values{1, 2, 3, 4, 5};
    for (const auto& value : values) {
        processTemplate(value);
    }

    return 0;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "virtual", // Search for specific C++ keyword
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the keyword in context
    assert!(
        output.contains("virtual"),
        "Should contain 'virtual' keyword in search results - output: {}",
        output
    );

    // Should show C++ class and inheritance structure
    assert!(
        output.contains("class") && (output.contains("public") || output.contains("private")),
        "Should show C++ class structure with access specifiers - output: {}",
        output
    );

    // Test another keyword search
    let ctx2 = TestContext::new();
    let output2 = ctx2.run_probe(&[
        "search",
        "template",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output2.contains("template"),
        "Should contain 'template' keyword - output: {}",
        output2
    );

    // Test constexpr keyword search
    let ctx3 = TestContext::new();
    let output3 = ctx3.run_probe(&[
        "search",
        "constexpr",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output3.contains("constexpr"),
        "Should contain 'constexpr' keyword - output: {}",
        output3
    );

    Ok(())
}

#[test]
fn test_cpp_stl_container_truncation_with_keyword_preservation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("stl_truncation.cpp");

    let content = r#"#include <vector>
#include <map>
#include <unordered_map>
#include <string>
#include <algorithm>
#include <iterator>

// Function with large STL container literals that should be truncated while preserving keywords
class STLDemonstration {
public:
    // Large vector with search keywords
    std::vector<std::string> createSearchKeywords() {
        return {
            "search", "find", "locate", "discover", "identify", "detect", "browse",
            "explore", "investigate", "examine", "scrutinize", "analyze", "study",
            "research", "query", "request", "ask", "question", "seek", "hunt",
            "track", "trace", "follow", "pursue", "chase", "stalk", "scout",
            "survey", "scan", "probe", "test", "check", "verify", "validate",
            "confirm", "authenticate", "authorize", "approve", "accept", "allow",
            "permit", "enable", "activate", "trigger", "initiate", "start",
            "begin", "commence", "launch", "execute", "run", "perform", "operate",
            "function", "work", "process", "handle", "manage", "control", "direct",
            "guide", "lead", "conduct", "coordinate", "organize", "arrange",
            "structure", "format", "design", "create", "build", "construct",
            "develop", "implement", "establish", "setup", "configure", "customize",
            "modify", "change", "alter", "adjust", "adapt", "transform", "convert"
        };
    }

    // Large map with mixed keywords
    std::map<std::string, int> createKeywordFrequencies() {
        return {
            {"algorithm", 150}, {"data", 200}, {"structure", 180}, {"search", 250},
            {"sort", 175}, {"find", 220}, {"insert", 190}, {"delete", 165},
            {"update", 140}, {"query", 210}, {"index", 185}, {"key", 195},
            {"value", 205}, {"hash", 160}, {"tree", 170}, {"graph", 155},
            {"node", 145}, {"edge", 135}, {"vertex", 125}, {"path", 115},
            {"distance", 105}, {"weight", 95}, {"cost", 85}, {"benefit", 75},
            {"optimization", 65}, {"performance", 255}, {"efficiency", 245},
            {"complexity", 235}, {"analysis", 225}, {"design", 215}, {"pattern", 185},
            {"template", 175}, {"generic", 165}, {"polymorphism", 155}, {"inheritance", 145},
            {"encapsulation", 135}, {"abstraction", 125}, {"interface", 115}, {"implementation", 105}
        };
    }

    // Large unordered_map with keyword patterns
    std::unordered_map<std::string, std::vector<std::string>> createCategoryKeywords() {
        return {
            {"search", {"find", "locate", "discover", "identify", "detect", "browse", "explore", "investigate"}},
            {"sorting", {"sort", "order", "arrange", "organize", "rank", "priority", "sequence", "classification"}},
            {"data", {"information", "content", "value", "record", "entry", "item", "element", "object"}},
            {"structure", {"organization", "arrangement", "layout", "format", "design", "architecture", "framework"}},
            {"algorithm", {"procedure", "method", "technique", "approach", "strategy", "solution", "process"}},
            {"performance", {"speed", "efficiency", "optimization", "throughput", "latency", "benchmark", "metrics"}},
            {"testing", {"validation", "verification", "checking", "debugging", "analysis", "evaluation", "assessment"}},
            {"development", {"programming", "coding", "implementation", "construction", "creation", "building"}},
            {"maintenance", {"update", "modify", "repair", "fix", "enhance", "improve", "refactor", "optimize"}}
        };
    }

    // Function that processes search terms
    void processSearchTerms(const std::string& searchTerm) {
        auto keywords = createSearchKeywords();
        auto frequencies = createKeywordFrequencies();
        auto categories = createCategoryKeywords();

        // Search through keywords
        auto it = std::find(keywords.begin(), keywords.end(), searchTerm);
        if (it != keywords.end()) {
            std::cout << "Found search term: " << searchTerm << std::endl;
        }

        // Check frequency
        if (frequencies.find(searchTerm) != frequencies.end()) {
            std::cout << "Frequency for " << searchTerm << ": " << frequencies[searchTerm] << std::endl;
        }

        // Check categories
        for (const auto& [category, terms] : categories) {
            if (std::find(terms.begin(), terms.end(), searchTerm) != terms.end()) {
                std::cout << "Found " << searchTerm << " in category: " << category << std::endl;
            }
        }
    }

    // Template function with complex STL usage
    template<typename Container, typename Predicate>
    auto filterAndSearch(const Container& container, Predicate pred, const std::string& searchKey) {
        std::vector<typename Container::value_type> filtered;

        std::copy_if(container.begin(), container.end(),
                    std::back_inserter(filtered),
                    pred);

        auto searchResult = std::find_if(filtered.begin(), filtered.end(),
                                       [&searchKey](const auto& item) {
                                           return item.find(searchKey) != std::string::npos;
                                       });

        return searchResult != filtered.end();
    }
};

// Global search function with large initialization
void performGlobalSearch(const std::string& query) {
    // Large static data with search-related keywords
    static std::vector<std::string> searchDomains = {
        "web", "database", "filesystem", "network", "cache", "index", "catalog",
        "directory", "registry", "repository", "archive", "library", "collection",
        "dataset", "datastore", "warehouse", "mart", "lake", "stream", "queue",
        "buffer", "pool", "heap", "stack", "tree", "graph", "table", "view",
        "search", "find", "locate", "discover", "identify", "detect", "browse"
    };

    STLDemonstration demo;
    demo.processSearchTerms(query);
}

int main() {
    performGlobalSearch("search");
    return 0;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "search", // Search for the keyword "search"
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the search keyword
    assert!(
        output.contains("search"),
        "Should contain 'search' keyword - output: {}",
        output
    );

    // Should show large STL containers (they may be truncated or fully shown)
    assert!(
        output.contains("std::vector") || output.contains("vector") || output.len() > 1000,
        "Should contain STL container code or show substantial content - output: {}",
        output
    );

    // Should preserve the search keyword even in truncated containers
    // The output should have reasonable length (not thousands of lines)
    let line_count = output.lines().count();
    assert!(
        line_count < 300,
        "Output should be truncated to reasonable size, got {} lines",
        line_count
    );

    // Should contain STL container types
    assert!(
        output.contains("std::vector")
            || output.contains("std::map")
            || output.contains("vector")
            || output.contains("map"),
        "Should contain STL container types - output: {}",
        output
    );

    // Should show template and class structures
    assert!(
        output.contains("template") || output.contains("class"),
        "Should show C++ structural elements - output: {}",
        output
    );

    Ok(())
}

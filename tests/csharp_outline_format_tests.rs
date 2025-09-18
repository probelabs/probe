use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_csharp_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("Calculator.cs");

    let content = r#"using System;
using System.Collections.Generic;
using System.Linq;

namespace Calculator
{
    public interface ICalculator
    {
        double Add(double x, double y);
        double Subtract(double x, double y);
    }

    public class BasicCalculator : ICalculator
    {
        public double Add(double x, double y) => x + y;
        public double Subtract(double x, double y) => x - y;
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
    ])?;

    // Verify C# symbols are found in outline format
    assert!(
        output.contains("ICalculator") || output.contains("BasicCalculator"),
        "Missing C# symbols - output: {}",
        output
    );
    assert!(
        output.contains("namespace Calculator"),
        "Missing namespace - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_csharp_outline_smart_closing_brace_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("smart_braces.cs");

    let content = r#"using System;
using System.Collections.Generic;
using System.Linq;

namespace SmartBraces
{
    /// Small method that should NOT get closing brace comments.
    public class SmallClass
    {
        public void SmallMethod(int x)
        {
            var result = x * 2;
            Console.WriteLine(result);
        }
    }

    /// Large class that SHOULD get closing brace comments when there are gaps.
    public class LargeClassWithGaps
    {
        private readonly List<string> _data = new List<string>();
        private const int MaxRetries = 3;

        // Phase 1: Initial setup methods
        public void InitializeData()
        {
            for (int i = 0; i < 100; i++)
            {
                if (i % 2 == 0)
                {
                    _data.Add($"Even: {i}");
                }
                else
                {
                    _data.Add($"Odd: {i}");
                }
            }
        }

        // Phase 2: Complex processing logic with nested structures
        public void ProcessDataWithComplexLogic()
        {
            var processedData = new Dictionary<string, int>();

            foreach (var item in _data)
            {
                if (item.StartsWith("Even"))
                {
                    var number = ExtractNumber(item);
                    if (number > 50)
                    {
                        processedData[item] = number * 2;
                    }
                    else
                    {
                        processedData[item] = number;
                    }
                }
                else if (item.StartsWith("Odd"))
                {
                    var number = ExtractNumber(item);
                    processedData[item] = number + 10;
                }
            }

            // Final validation and cleanup
            var validatedData = new List<KeyValuePair<string, int>>();
            foreach (var kvp in processedData)
            {
                if (kvp.Value > 0 && kvp.Key.Length > 5)
                {
                    validatedData.Add(kvp);
                }
            }
        }

        // Phase 3: Helper methods and utilities
        private int ExtractNumber(string input)
        {
            var parts = input.Split(':');
            if (parts.Length > 1 && int.TryParse(parts[1].Trim(), out var number))
            {
                return number;
            }
            return 0;
        }
    }

    /// Another large class to test closing brace behavior with control flow
    public class ControlFlowClass
    {
        public void ProcessWithControlFlow(List<Item> items)
        {
            var accumulator = new Dictionary<ItemType, List<Item>>();

            // Main processing loop with complex nested control flow
            foreach (var item in items)
            {
                try
                {
                    switch (item.Type)
                    {
                        case ItemType.Primary:
                            if (item.Weight > 50.0)
                            {
                                if (!accumulator.ContainsKey(ItemType.Primary))
                                    accumulator[ItemType.Primary] = new List<Item>();
                                accumulator[ItemType.Primary].Add(item);
                            }
                            else
                            {
                                // Handle lightweight primary items
                                ProcessLightweightItem(item);
                            }
                            break;
                        case ItemType.Secondary:
                            ProcessSecondaryItem(item, accumulator);
                            break;
                        case ItemType.Auxiliary:
                            ProcessAuxiliaryItem(item);
                            break;
                        default:
                            throw new ArgumentException($"Unknown item type: {item.Type}");
                    }
                }
                catch (Exception ex)
                {
                    Console.WriteLine($"Error processing item {item.Id}: {ex.Message}");
                    continue;
                }
            }

            FinalizeProcessing(accumulator);
        }

        private void ProcessLightweightItem(Item item) { }
        private void ProcessSecondaryItem(Item item, Dictionary<ItemType, List<Item>> acc) { }
        private void ProcessAuxiliaryItem(Item item) { }
        private void FinalizeProcessing(Dictionary<ItemType, List<Item>> acc) { }
    }

    public class Item
    {
        public int Id { get; set; }
        public ItemType Type { get; set; }
        public double Weight { get; set; }
    }

    public enum ItemType
    {
        Primary,
        Secondary,
        Auxiliary
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "InitializeData",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the method
    assert!(
        output.contains("InitializeData"),
        "Missing InitializeData method - output: {}",
        output
    );

    // Should contain the class and method structure in outline format
    assert!(
        output.contains("LargeClassWithGaps") || output.contains("public void InitializeData"),
        "Should contain class or method declaration - output: {}",
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:") || output.contains("File:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_csharp_outline_keyword_highlighting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keywords.cs");

    let content = r#"using System;
using System.Linq;
using System.Collections.Generic;

namespace KeywordTest
{
    public class KeywordProcessor
    {
        // Methods containing various C# keywords
        public async Task ProcessAsync()
        {
            var data = await GetDataAsync();
            foreach (var item in data.Where(x => x != null))
            {
                if (item is string text && !string.IsNullOrEmpty(text))
                {
                    yield return text.ToUpper();
                }
            }
        }

        public void ProcessWithPatternMatching(object input)
        {
            var result = input switch
            {
                string s when s.Length > 10 => $"Long string: {s}",
                int i when i > 0 => $"Positive number: {i}",
                null => "Null value",
                _ => "Unknown type"
            };
        }

        public void UsingStatement()
        {
            using var resource = new DisposableResource();
            using (var connection = new DatabaseConnection())
            {
                connection.Execute("SELECT * FROM table");
            }
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "ProcessAsync",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should contain the async method
    assert!(
        output.contains("ProcessAsync"),
        "Should contain ProcessAsync method - output: {}",
        output
    );

    // Should contain Task in the return type or await keyword
    assert!(
        output.contains("Task") || output.contains("await"),
        "Should contain async-related keywords - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_csharp_outline_testing_frameworks_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("tests.cs");

    let content = r#"using System;
using Xunit;
using NUnit.Framework;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Calculator.Tests
{
    // xUnit tests
    public class XUnitCalculatorTests
    {
        [Fact]
        public void Add_ShouldReturnSum_WhenGivenTwoNumbers()
        {
            var calc = new Calculator();
            var result = calc.Add(2, 3);
            Assert.Equal(5, result);
        }

        [Theory]
        [InlineData(2, 3, 5)]
        [InlineData(0, 0, 0)]
        [InlineData(-1, 1, 0)]
        public void Add_ShouldReturnCorrectSum_ForVariousInputs(int a, int b, int expected)
        {
            var calc = new Calculator();
            var result = calc.Add(a, b);
            Assert.Equal(expected, result);
        }
    }

    // NUnit tests
    [TestFixture]
    public class NUnitCalculatorTests
    {
        private Calculator _calculator;

        [SetUp]
        public void SetUp()
        {
            _calculator = new Calculator();
        }

        [Test]
        public void Multiply_ShouldReturnProduct_WhenGivenTwoNumbers()
        {
            var result = _calculator.Multiply(3, 4);
            Assert.AreEqual(12, result);
        }

        [TestCase(2, 3, 6)]
        [TestCase(0, 5, 0)]
        [TestCase(-2, 3, -6)]
        public void Multiply_ShouldReturnCorrectProduct_ForVariousInputs(int a, int b, int expected)
        {
            var result = _calculator.Multiply(a, b);
            Assert.AreEqual(expected, result);
        }
    }

    // MSTest tests
    [TestClass]
    public class MSTestCalculatorTests
    {
        private Calculator _calculator;

        [TestInitialize]
        public void Initialize()
        {
            _calculator = new Calculator();
        }

        [TestMethod]
        public void Divide_ShouldReturnQuotient_WhenGivenTwoNumbers()
        {
            var result = _calculator.Divide(10, 2);
            Assert.AreEqual(5, result);
        }

        [DataTestMethod]
        [DataRow(10, 2, 5)]
        [DataRow(15, 3, 5)]
        [DataRow(0, 1, 0)]
        public void Divide_ShouldReturnCorrectQuotient_ForVariousInputs(int a, int b, int expected)
        {
            var result = _calculator.Divide(a, b);
            Assert.AreEqual(expected, result);
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // Test xUnit detection
    let output = ctx.run_probe(&[
        "search",
        "Fact",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("Fact") || output.contains("[Fact]"),
        "Should detect xUnit [Fact] attribute - output: {}",
        output
    );

    // Test NUnit detection
    let output = ctx.run_probe(&[
        "search",
        "TestFixture",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("TestFixture") || output.contains("[TestFixture]"),
        "Should detect NUnit [TestFixture] attribute - output: {}",
        output
    );

    // Test MSTest detection
    let output = ctx.run_probe(&[
        "search",
        "TestMethod",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("TestMethod") || output.contains("[TestMethod]"),
        "Should detect MSTest [TestMethod] attribute - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_csharp_outline_modern_features() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("modern_csharp.cs");

    let content = r#"using System;
using System.Collections.Generic;
using System.Linq;

namespace ModernCSharp
{
    // Records (C# 9+)
    public record Person(string Name, int Age)
    {
        public string? Email { get; init; }
    }

    public record struct Point(double X, double Y);

    // Nullable reference types
    public class DataProcessor
    {
        public string? ProcessData(string? input)
        {
            if (input is null)
                return null;

            return input.ToUpper();
        }

        public List<string> FilterNonNullItems(List<string?> items)
        {
            return items.Where(x => x is not null).ToList()!;
        }
    }

    // Pattern matching enhancements
    public class PatternMatchingExamples
    {
        public string DescribeValue(object value) => value switch
        {
            null => "It's null",
            string s when s.Length == 0 => "Empty string",
            string s => $"String of length {s.Length}",
            int i when i < 0 => "Negative number",
            int i when i == 0 => "Zero",
            int i => $"Positive number: {i}",
            Person { Age: >= 18 } p => $"Adult: {p.Name}",
            Person p => $"Minor: {p.Name}",
            IEnumerable<string> list => $"Collection with {list.Count()} items",
            _ => "Unknown type"
        };

        public bool IsValidPerson(Person person) => person switch
        {
            { Name: not null, Age: >= 0 and < 150 } => true,
            _ => false
        };
    }

    // Top-level programs support (would typically be in Program.cs)
    public static class TopLevelDemo
    {
        public static void RunDemo(string[] args)
        {
            Console.WriteLine("Hello, World!");

            var person = new Person("John", 30) { Email = "john@example.com" };
            Console.WriteLine($"Person: {person}");

            var point = new Point(1.0, 2.0);
            Console.WriteLine($"Point: {point}");

            var processor = new DataProcessor();
            var result = processor.ProcessData("test");
            Console.WriteLine($"Result: {result ?? "null"}");
        }
    }

    // Target-typed new expressions
    public class ModernSyntax
    {
        private readonly List<Person> _people = new();
        private readonly Dictionary<string, int> _scores = new()
        {
            ["Alice"] = 95,
            ["Bob"] = 87
        };

        public void InitializationExamples()
        {
            Point point = new(1.0, 2.0);
            Person person = new("Jane", 25);

            var list = new List<string> { "item1", "item2" };
        }
    }

    // Switch expressions and property patterns
    public class AdvancedPatterns
    {
        public decimal CalculateDiscount(Person customer, decimal amount) =>
            (customer, amount) switch
            {
                ({ Age: >= 65 }, _) => amount * 0.1m,
                (_, >= 1000) => amount * 0.05m,
                ({ Name: "VIP" }, _) => amount * 0.15m,
                _ => 0m
            };

        public string GetDayType(DateTime date) => date.DayOfWeek switch
        {
            DayOfWeek.Saturday or DayOfWeek.Sunday => "Weekend",
            DayOfWeek.Monday => "Start of week",
            DayOfWeek.Friday => "End of week",
            _ => "Weekday"
        };
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // Test records
    let output = ctx.run_probe(&[
        "search",
        "record",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("record") && (output.contains("Person") || output.contains("Point")),
        "Should detect C# records - output: {}",
        output
    );

    // Test nullable reference types
    let output = ctx.run_probe(&[
        "search",
        "string?",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("string?") || output.contains("nullable"),
        "Should detect nullable reference types - output: {}",
        output
    );

    // Test pattern matching with a more specific search
    let output = ctx.run_probe(&[
        "search",
        "DescribeValue",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("DescribeValue") && (output.contains("switch") || output.contains("=>")),
        "Should detect pattern matching method - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_csharp_outline_linq_and_generics() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("linq_generics.cs");

    let content = r#"using System;
using System.Collections.Generic;
using System.Linq;

namespace LinqAndGenerics
{
    public class Repository<T> where T : class, IEntity
    {
        private readonly List<T> _items = new List<T>();

        public void Add(T item)
        {
            _items.Add(item);
        }

        public IEnumerable<T> FindByPredicate(Func<T, bool> predicate)
        {
            return _items.Where(predicate);
        }

        public TResult? Transform<TResult>(T item, Func<T, TResult> transformer)
            where TResult : class
        {
            return item != null ? transformer(item) : null;
        }
    }

    public interface IEntity
    {
        int Id { get; }
        string Name { get; }
    }

    public class Customer : IEntity
    {
        public int Id { get; set; }
        public string Name { get; set; } = string.Empty;
        public string Email { get; set; } = string.Empty;
    }

    public class LinqOperations
    {
        public void ComplexLinqQuery(List<Customer> customers)
        {
            var result = customers
                .Where(c => c.Email.Contains("@gmail.com"))
                .GroupBy(c => c.Name.Substring(0, 1))
                .Select(g => new
                {
                    FirstLetter = g.Key,
                    Count = g.Count(),
                    Customers = g.Select(c => c.Name).ToList()
                })
                .OrderBy(x => x.FirstLetter)
                .ToList();

            var emailDomains = customers
                .Select(c => c.Email.Split('@').LastOrDefault())
                .Where(domain => !string.IsNullOrEmpty(domain))
                .GroupBy(domain => domain)
                .ToDictionary(g => g.Key, g => g.Count());
        }

        public IEnumerable<TResult> ProcessItems<TSource, TResult>(
            IEnumerable<TSource> source,
            Func<TSource, bool> filter,
            Func<TSource, TResult> selector)
        {
            return source.Where(filter).Select(selector);
        }
    }

    // Delegates and Events
    public delegate void DataChangedEventHandler<T>(T oldValue, T newValue);

    public class DataContainer<T>
    {
        private T _value = default(T)!;

        public event DataChangedEventHandler<T>? ValueChanged;
        public event Action<string>? LogMessage;

        public T Value
        {
            get => _value;
            set
            {
                var oldValue = _value;
                _value = value;
                OnValueChanged(oldValue, value);
                LogMessage?.Invoke($"Value changed from {oldValue} to {value}");
            }
        }

        protected virtual void OnValueChanged(T oldValue, T newValue)
        {
            ValueChanged?.Invoke(oldValue, newValue);
        }
    }

    // Properties and Indexers
    public class SmartCollection<T>
    {
        private readonly Dictionary<string, T> _items = new();

        public T this[string key]
        {
            get => _items.TryGetValue(key, out var value) ? value : default(T)!;
            set => _items[key] = value;
        }

        public int Count => _items.Count;

        public IEnumerable<string> Keys => _items.Keys;

        public bool IsEmpty => _items.Count == 0;
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // Test LINQ detection
    let output = ctx.run_probe(&[
        "search",
        "Where",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("Where")
            && (output.contains("Select") || output.contains("LINQ") || output.contains("linq")),
        "Should detect LINQ operations - output: {}",
        output
    );

    // Test generics
    let output = ctx.run_probe(&[
        "search",
        "Repository",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("Repository") && output.contains("<T>"),
        "Should detect generic types - output: {}",
        output
    );

    // Test delegates
    let output = ctx.run_probe(&[
        "search",
        "delegate",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("delegate") || output.contains("DataChangedEventHandler"),
        "Should detect delegates - output: {}",
        output
    );

    // Test events with more specific search
    let output = ctx.run_probe(&[
        "search",
        "ValueChanged",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    assert!(
        output.contains("ValueChanged") || output.contains("DataChangedEventHandler"),
        "Should detect events - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_csharp_outline_array_truncation_with_keyword_preservation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_arrays.cs");

    let content = r#"using System;
using System.Collections.Generic;
using System.Linq;

namespace ArrayTruncation
{
    public class ConfigurationManager
    {
        /// Configuration data that should be truncated but preserve the 'important' keyword.
        public static readonly string[] ImportantConfiguration = {
            "setting1=value1",
            "setting2=value2",
            "important=critical_value",
            "setting3=value3",
            "setting4=value4",
            "setting5=value5",
            "setting6=value6",
            "setting7=value7",
            "setting8=value8",
            "setting9=value9",
            "setting10=value10",
            "setting11=value11",
            "setting12=value12",
            "important_backup=backup_value",
            "setting13=value13",
            "setting14=value14",
            "setting15=value15",
            "setting16=value16",
            "setting17=value17",
            "setting18=value18",
            "setting19=value19",
            "setting20=value20",
            "final_important_setting=final_value"
        };

        public void ProcessLargeDataSet()
        {
            var keywords = new List<string> {
                "abstract", "as", "base", "bool", "break", "byte", "case", "catch", "char",
                "checked", "class", "const", "continue", "decimal", "default", "delegate",
                "do", "double", "else", "enum", "event", "explicit", "extern", "false",
                "finally", "fixed", "float", "for", "foreach", "goto", "if", "implicit",
                "in", "int", "interface", "internal", "is", "lock", "long", "namespace",
                "new", "null", "object", "operator", "out", "override", "params", "private",
                "protected", "public", "readonly", "ref", "return", "sbyte", "sealed",
                "short", "sizeof", "stackalloc", "static", "string", "struct", "switch",
                "this", "throw", "true", "try", "typeof", "uint", "ulong", "unchecked",
                "unsafe", "ushort", "using", "virtual", "void", "volatile", "while"
            };

            var filteredKeywords = keywords.Where(k => k.Contains("important")).ToList();
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "important",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the keyword even in truncated arrays
    assert!(
        output.contains("important"),
        "Should preserve 'important' keyword in truncated arrays - output: {}",
        output
    );

    // Should show truncation with ellipsis
    assert!(
        output.contains("...") || output.contains("/*"),
        "Should show array truncation - output: {}",
        output
    );

    // Should have reasonable length (not show entire massive array)
    let line_count = output.lines().count();
    assert!(
        line_count < 100,
        "Output should be truncated to reasonable size, got {} lines",
        line_count
    );

    Ok(())
}

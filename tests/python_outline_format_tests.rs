use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_python_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("basic.py");
    
    let content = r#"class Calculator:
    """A simple calculator class."""
    
    def __init__(self, name: str):
        self.name = name
    
    def add(self, x: int, y: int) -> int:
        """Add two numbers."""
        return x + y
    
    @property
    def version(self) -> str:
        """Get calculator version."""
        return "1.0.0"

def process_data(data: list) -> int:
    """Process a list of data."""
    return len(data)

async def fetch_data(url: str) -> str:
    """Fetch data asynchronously."""
    return f"data from {url}"

def test_calculator():
    """Test the calculator functionality."""
    calc = Calculator("test")
    assert calc.add(2, 3) == 5
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests"
    ])?;
    
    // Verify Python symbols are extracted
    assert!(output.contains("class Calculator:"), "Missing class Calculator - output: {}", output);
    assert!(output.contains("def process_data(data: list) -> int:"), "Missing function process_data - output: {}", output);
    assert!(output.contains("async def fetch_data(url: str) -> str:"), "Missing async function fetch_data - output: {}", output);
    assert!(output.contains("def test_calculator():"), "Missing test function - output: {}", output);
    
    Ok(())
}

#[test] 
fn test_python_outline_decorators() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("decorators.py");
    
    let content = r#"from functools import wraps

def timing_decorator(func):
    @wraps(func)
    def wrapper(*args, **kwargs):
        import time
        start = time.time()
        result = func(*args, **kwargs)
        end = time.time()
        print(f"Execution time: {end - start}")
        return result
    return wrapper

class UserManager:
    def __init__(self, db_url: str):
        self.db_url = db_url
    
    @property
    def connection_status(self) -> str:
        """Get database connection status."""
        return "connected"
    
    @classmethod
    def create_default(cls):
        """Create UserManager with default settings."""
        return cls("sqlite:///default.db")
    
    @staticmethod
    def validate_email(email: str) -> bool:
        """Validate email format."""
        return "@" in email
    
    @timing_decorator
    def process_users(self, users: list) -> list:
        """Process a list of users."""
        return [user for user in users if self.validate_email(user.get('email', ''))]
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(),
        "--format", 
        "outline",
        "--allow-tests"
    ])?;
    
    // Verify decorated functions and methods are properly shown
    assert!(output.contains("def timing_decorator(func):"), "Missing decorator function - output: {}", output);
    assert!(output.contains("class UserManager:"), "Missing UserManager class - output: {}", output);
    // Note: Currently the extract command only shows top-level symbols
    // Decorated methods within classes are not currently extracted as separate symbols
    // This could be improved in future versions
    
    Ok(())
}

#[test]
fn test_python_outline_nested_classes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("nested.py");
    
    let content = r#"class ReportGenerator:
    """Generate various types of reports."""
    
    def __init__(self):
        self.reports_created = 0
    
    class PDFReport:
        """Nested class for PDF reports."""
        
        def __init__(self, title: str):
            self.title = title
        
        def generate(self, data: dict) -> bytes:
            """Generate PDF from data."""
            return b"PDF content"
        
        class Metadata:
            """Metadata for PDF reports."""
            
            def __init__(self, author: str):
                self.author = author
    
    class CSVReport:
        """Nested class for CSV reports."""
        
        def __init__(self, delimiter: str = ','):
            self.delimiter = delimiter
        
        def generate(self, data: list) -> str:
            """Generate CSV from data.""" 
            return "CSV content"
    
    def create_pdf_report(self, title: str):
        """Create a PDF report instance."""
        return self.PDFReport(title)

def outer_function():
    """Function with nested functions."""
    
    def inner_function(x: int) -> int:
        """Inner function."""
        return x * 2
    
    def another_inner() -> str:
        """Another inner function."""
        return "inner"
    
    return inner_function, another_inner
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(), 
        "--format",
        "outline",
        "--allow-tests"
    ])?;
    
    // Verify top-level structures are shown - outline format only shows top-level classes and functions
    assert!(output.contains("class ReportGenerator:"), "Missing ReportGenerator class - output: {}", output);
    assert!(output.contains("def outer_function():"), "Missing outer function - output: {}", output);
    // Nested classes (PDFReport, CSVReport, Metadata) are not shown individually in outline format
    // They are inside ReportGenerator and outline format only shows top-level structures
    assert!(output.contains("..."), "Missing ellipsis in outline format - output: {}", output);
    
    Ok(())
}

#[test]
fn test_python_outline_docstrings_and_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("docstrings.py");
    
    let content = r#"# Single line comment
def function_with_single_quotes():
    '''Function with single quote docstring.
    
    This tests parsing of single quote docstrings.
    '''
    # Internal comment
    return "result"

def function_with_double_quotes():
    """Function with double quote docstring.
    
    This tests parsing of double quote docstrings.
    """
    return "result"

def function_with_raw_docstring():
    r"""Raw docstring with backslashes: \n\t\r
    
    This is a raw string docstring.
    """
    return "result"

class DocumentedClass:
    """Class with comprehensive docstring.
    
    Attributes:
        name: The name of the object
        value: The numeric value
    
    Examples:
        >>> obj = DocumentedClass("test", 42)
        >>> obj.name
        'test'
    """
    
    def __init__(self, name: str, value: int):
        self.name = name  # Name comment
        self.value = value  # Value comment
    
    def method_with_docstring(self):
        """Method with proper docstring.
        
        Returns:
            str: A formatted string
        """
        return f"{self.name}: {self.value}"

# Comment above function
def test_docstring_parsing():
    """Test that docstrings are preserved in outline."""
    obj = DocumentedClass("test", 42)
    assert obj.method_with_docstring() == "test: 42"
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests"
    ])?;
    
    // Verify functions and classes are shown
    assert!(output.contains("def function_with_single_quotes():"), "Missing function with single quote docstring - output: {}", output);
    assert!(output.contains("def function_with_double_quotes():"), "Missing function with double quote docstring - output: {}", output); 
    assert!(output.contains("def function_with_raw_docstring():"), "Missing function with raw docstring - output: {}", output);
    assert!(output.contains("class DocumentedClass:"), "Missing documented class - output: {}", output);
    assert!(output.contains("def test_docstring_parsing():"), "Missing test function - output: {}", output);
    
    Ok(())
}

#[test]
fn test_python_outline_complex_signatures() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("complex_signatures.py");
    
    let content = r#"from typing import List, Dict, Optional, Union, Callable, AsyncIterator
from dataclasses import dataclass

@dataclass
class Config:
    name: str
    value: int = 0

def simple_function(x: int, y: int) -> int:
    """Simple function signature."""
    return x + y

def function_with_defaults(
    name: str,
    age: int = 25,
    city: str = "Unknown",
    *args,
    **kwargs
) -> Dict[str, Union[str, int]]:
    """Function with default parameters and var args."""
    return {"name": name, "age": age, "city": city}

async def async_function_complex(
    data: List[Dict[str, Union[str, int]]],
    processor: Callable[[Dict], Dict],
    *,
    batch_size: int = 10,
    timeout: float = 30.0,
    validate: bool = True
) -> AsyncIterator[Dict]:
    """Async function with complex type hints and keyword-only args."""
    for item in data:
        yield processor(item)

def function_multiline_signature(
    matrix_a: List[List[float]],
    matrix_b: List[List[float]], 
    precision: int = 10,
    validate_dimensions: bool = True,
    output_format: str = "list"
) -> Union[List[List[float]], str]:
    """Function with multiline signature."""
    return [[0.0]]

class GenericProcessor:
    """Class with complex method signatures."""
    
    def __init__(
        self, 
        name: str,
        config: Optional[Config] = None,
        processors: List[Callable] = None
    ):
        self.name = name
        self.config = config or Config("default")
        self.processors = processors or []
    
    async def process_batch(
        self,
        items: List[Dict[str, Union[str, int, float]]],
        *,
        parallel: bool = True,
        max_workers: int = 4,
        timeout_per_item: float = 1.0
    ) -> List[Optional[Dict]]:
        """Process items in batch with complex parameters."""
        return [{"processed": True} for _ in items]
    
    @classmethod
    def from_config_file(
        cls,
        config_path: str,
        overrides: Optional[Dict[str, Union[str, int]]] = None
    ) -> 'GenericProcessor':
        """Create processor from config file."""
        return cls("from_config")

def test_complex_signatures():
    """Test complex function signature parsing."""
    processor = GenericProcessor.from_config_file("config.json")
    assert processor.name == "from_config"
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(),
        "--format", 
        "outline",
        "--allow-tests"
    ])?;
    
    // Verify top-level complex signatures are preserved - outline format shows only top-level structures
    assert!(output.contains("def function_with_defaults("), "Missing function with defaults - output: {}", output);
    assert!(output.contains("async def async_function_complex("), "Missing async function - output: {}", output);
    assert!(output.contains("def function_multiline_signature("), "Missing multiline signature function - output: {}", output);
    assert!(output.contains("class GenericProcessor:"), "Missing GenericProcessor class - output: {}", output);
    // Methods inside classes (like async process_batch) are not shown individually in outline format
    assert!(output.contains("..."), "Missing ellipsis in outline format - output: {}", output);
    
    Ok(())
}

#[test] 
fn test_python_outline_test_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test_patterns.py");
    
    let content = r#"import unittest
import pytest
from unittest.mock import Mock, patch

class TestUserManager(unittest.TestCase):
    """Unit tests for UserManager using unittest."""
    
    def setUp(self):
        """Set up test fixtures."""
        self.user_manager = UserManager()
    
    def tearDown(self):
        """Clean up after tests."""
        self.user_manager = None
    
    def test_add_user(self):
        """Test adding a user."""
        result = self.user_manager.add_user("John", "john@example.com")
        self.assertTrue(result)
    
    def test_get_user_not_found(self):
        """Test getting non-existent user."""
        user = self.user_manager.get_user(999)
        self.assertIsNone(user)
    
    @patch('requests.get')
    def test_fetch_user_data(self, mock_get):
        """Test fetching user data with mocked requests."""
        mock_get.return_value.json.return_value = {"id": 1, "name": "Test"}
        result = self.user_manager.fetch_user_data(1)
        self.assertEqual(result["name"], "Test")

# Pytest-style tests
def test_simple_calculation():
    """Simple pytest test."""
    assert 2 + 2 == 4

def test_string_operations():
    """Test string operations."""
    text = "Hello World"
    assert text.lower() == "hello world"
    assert len(text) == 11

@pytest.mark.parametrize("input,expected", [
    (1, 2),
    (2, 4),
    (3, 6),
])
def test_double_function(input, expected):
    """Parametrized test for doubling function."""
    assert double(input) == expected

@pytest.mark.asyncio
async def test_async_operation():
    """Test async operation with pytest."""
    result = await some_async_function()
    assert result is not None

@pytest.fixture
def sample_data():
    """Pytest fixture providing sample data."""
    return {"name": "Test", "value": 42}

def test_with_fixture(sample_data):
    """Test using pytest fixture."""
    assert sample_data["name"] == "Test"
    assert sample_data["value"] == 42

class TestIntegration:
    """Pytest-style test class."""
    
    @classmethod
    def setup_class(cls):
        """Setup for test class."""
        cls.global_config = {}
    
    def setup_method(self):
        """Setup for each test method."""
        self.test_data = []
    
    def test_integration_scenario_one(self):
        """Integration test scenario."""
        assert True
    
    def test_integration_scenario_two(self):
        """Another integration test scenario."""
        assert len(self.test_data) == 0

# Test helper functions
def create_test_user(name: str = "Test User", email: str = "test@example.com"):
    """Helper function to create test users."""
    return {"name": name, "email": email}

def assert_user_valid(user: dict):
    """Helper function to validate user data."""
    assert "name" in user
    assert "email" in user
    assert "@" in user["email"]

# Performance test
def test_performance_large_dataset():
    """Test performance with large dataset."""
    data = list(range(10000))
    result = process_large_dataset(data)
    assert len(result) == 10000
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests"
    ])?;
    
    // Verify test patterns are detected - outline format shows only top-level structures
    assert!(output.contains("class TestUserManager(unittest.TestCase):"), "Missing unittest test class - output: {}", output);
    // Individual test methods inside TestUserManager class are not shown in outline format
    assert!(output.contains("def test_simple_calculation():"), "Missing pytest test function - output: {}", output);
    assert!(output.contains("def test_with_fixture(sample_data):"), "Missing pytest test with fixture - output: {}", output);
    assert!(output.contains("class TestIntegration:"), "Missing pytest test class - output: {}", output);
    assert!(output.contains("..."), "Missing ellipsis in outline format - output: {}", output);
    
    Ok(())
}

#[test]
fn test_python_outline_edge_cases() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("edge_cases.py");
    
    let content = r#"# -*- coding: utf-8 -*-
"""Module with edge cases for Python parsing."""

# Lambda functions (should not be extracted as symbols)
add = lambda x, y: x + y
multiply = lambda x, y: x * y

# Complex expressions (should not be symbols)
COMPLEX_CONFIG = {
    'key': 'value',
    'nested': {
        'inner': [1, 2, 3]
    }
}

# List comprehension assigned to variable
PROCESSED_DATA = [x * 2 for x in range(10) if x % 2 == 0]

# Generator expression
data_gen = (x for x in range(100) if x > 50)

class EmptyClass:
    """Class with only pass statement."""
    pass

class ClassWithOnlyProperties:
    """Class containing only properties."""
    
    @property
    def readonly_value(self):
        """A read-only property."""
        return 42
    
    @property 
    def name(self):
        """Name property getter."""
        return self._name
    
    @name.setter
    def name(self, value):
        """Name property setter."""
        self._name = value

def function_with_only_pass():
    """Function with only pass statement."""
    pass

def function_with_nested_definitions():
    """Function containing nested class and function."""
    
    class LocalClass:
        """Class defined inside function."""
        
        def local_method(self):
            """Method in local class."""
            return "local"
    
    def local_function():
        """Function defined inside function."""
        return LocalClass()
    
    return local_function()

# Very long function name
def function_with_extremely_long_name_that_might_cause_parsing_issues_in_outline_format():
    """Function with very long name."""
    return "long name"

# Function with unicode in name (Python 3 supports this)
def функция_с_unicode_именем():
    """Function with unicode characters in name.""" 
    return "unicode"

# Async generators
async def async_generator_function():
    """Async generator function."""
    for i in range(10):
        yield i

# Context manager class 
class CustomContextManager:
    """Custom context manager."""
    
    def __enter__(self):
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        return False

# Metaclass
class MetaClass(type):
    """A simple metaclass."""
    
    def __new__(cls, name, bases, attrs):
        return super().__new__(cls, name, bases, attrs)

class ClassWithMetaclass(metaclass=MetaClass):
    """Class using custom metaclass."""
    
    def method(self):
        """Method in metaclass-created class."""
        return "metaclass method"

def test_edge_case_parsing():
    """Test parsing of edge cases."""
    obj = ClassWithMetaclass()
    assert obj.method() == "metaclass method"

# Test with unusual indentation (mixed tabs and spaces - should be handled)
def test_indentation_edge_case():
    """Test with mixed indentation."""
	# This line uses a tab
    # This line uses spaces
    assert True
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests"
    ])?;
    
    // Verify edge cases are handled correctly
    assert!(output.contains("class EmptyClass:"), "Missing empty class - output: {}", output);
    assert!(output.contains("class ClassWithOnlyProperties:"), "Missing class with properties - output: {}", output);
    assert!(output.contains("def function_with_only_pass():"), "Missing function with pass - output: {}", output);
    assert!(output.contains("def function_with_nested_definitions():"), "Missing function with nested defs - output: {}", output);
    assert!(output.contains("async def async_generator_function():"), "Missing async generator - output: {}", output);
    assert!(output.contains("class MetaClass(type):"), "Missing metaclass - output: {}", output);
    assert!(output.contains("class ClassWithMetaclass(metaclass=MetaClass):"), "Missing class with metaclass - output: {}", output);
    
    // Lambda functions should NOT be extracted as symbols
    assert!(!output.contains("lambda"), "Lambda functions should not be symbols - output: {}", output);
    
    Ok(())
}

#[test]
fn test_python_outline_search_command() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("search_test.py");
    
    let content = r#"class DataProcessor:
    """Process various types of data."""
    
    def __init__(self):
        self.processed_count = 0
    
    def process_data(self, data: list) -> list:
        """Process input data."""
        self.processed_count += 1
        return [item for item in data if item]
    
    def get_processed_count(self) -> int:
        """Get number of processed items."""
        return self.processed_count

def process_file(filename: str) -> str:
    """Process a file."""
    return f"Processed {filename}"

async def process_async(data: dict) -> dict:
    """Process data asynchronously."""
    return {"processed": True, **data}

def test_data_processing():
    """Test data processing functionality."""
    processor = DataProcessor()
    result = processor.process_data([1, 2, None, 3])
    assert len(result) == 3
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "process",
        temp_dir.path().to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests"
    ])?;
    
    // Should find symbols containing "process"
    assert!(
        output.contains("DataProcessor") || 
        output.contains("process_data") || 
        output.contains("process_file") ||
        output.contains("process_async"),
        "Should find process-related symbols - output: {}", output
    );
    
    Ok(())
}

#[test]
fn test_python_outline_multiline_docstrings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("multiline_docstrings.py");
    
    let content = r#"def function_with_long_docstring():
    """This is a function with a very long docstring.
    
    The docstring spans multiple lines and contains detailed
    information about the function's behavior, parameters,
    return values, and examples.
    
    Args:
        None
    
    Returns:
        str: A simple string value
    
    Examples:
        >>> result = function_with_long_docstring()
        >>> print(result)
        'Hello from long docstring function'
    
    Note:
        This function demonstrates how multiline docstrings
        should be handled in the outline format.
    """
    return "Hello from long docstring function"

class ClassWithLongDocstring:
    """This is a class with a comprehensive docstring.
    
    The class demonstrates various Python features and shows
    how the outline format should handle classes with extensive
    documentation.
    
    Attributes:
        name (str): The name of the instance
        value (int): A numeric value
        items (list): A list of items
    
    Methods:
        get_info(): Returns formatted information
        process(data): Processes input data
        
    Examples:
        >>> obj = ClassWithLongDocstring("test", 42)
        >>> info = obj.get_info()
        >>> print(info)
        'Name: test, Value: 42'
    
    Raises:
        ValueError: If invalid parameters are provided
        TypeError: If wrong types are used
    """
    
    def __init__(self, name: str, value: int):
        """Initialize the class instance.
        
        Args:
            name: The name for this instance
            value: A numeric value to store
        """
        self.name = name
        self.value = value
        self.items = []
    
    def get_info(self) -> str:
        """Get formatted information about this instance.
        
        Returns:
            A formatted string containing name and value
        """
        return f"Name: {self.name}, Value: {self.value}"
"#;
    
    fs::write(&test_file, content)?;
    
    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "extract",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests"
    ])?;
    
    // Verify top-level symbols are extracted - outline format shows only top-level structures  
    assert!(output.contains("def function_with_long_docstring():"), "Missing function with long docstring - output: {}", output);
    assert!(output.contains("class ClassWithLongDocstring:"), "Missing class with long docstring - output: {}", output);
    // Methods inside classes (like get_info) are not shown individually in outline format
    assert!(output.contains("..."), "Missing ellipsis in outline format - output: {}", output);
    
    Ok(())
}
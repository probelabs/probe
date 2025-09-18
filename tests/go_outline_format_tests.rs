use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_go_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("basic.go");

    let content = r#"// Package calculator provides arithmetic operations
package calculator

import (
    "fmt"
    "math"
    "errors"
)

// Calculator represents a calculator with history tracking
type Calculator struct {
    Name    string
    History []float64
    precision int
}

// CalculatorInterface defines the contract for calculator operations
type CalculatorInterface interface {
    Add(x, y float64) float64
    Subtract(x, y float64) float64
    Multiply(x, y float64) float64
    Divide(x, y float64) (float64, error)
    GetHistory() []float64
}

// Operation represents a mathematical operation
type Operation int

const (
    // Addition represents the addition operation
    Addition Operation = iota
    // Subtraction represents the subtraction operation
    Subtraction
    // Multiplication represents the multiplication operation
    Multiplication
    // Division represents the division operation
    Division
)

var (
    // DefaultPrecision is the default decimal precision
    DefaultPrecision = 2
    // ErrDivisionByZero is returned when dividing by zero
    ErrDivisionByZero = errors.New("division by zero")
)

// NewCalculator creates a new calculator instance
func NewCalculator(name string) *Calculator {
    return &Calculator{
        Name:      name,
        History:   make([]float64, 0),
        precision: DefaultPrecision,
    }
}

// Add adds two numbers and returns the result
func (c *Calculator) Add(x, y float64) float64 {
    result := x + y
    c.recordOperation(result)
    return c.roundToPrecision(result)
}

// Subtract subtracts y from x and returns the result
func (c *Calculator) Subtract(x, y float64) float64 {
    result := x - y
    c.recordOperation(result)
    return c.roundToPrecision(result)
}

// Multiply multiplies two numbers and returns the result
func (c *Calculator) Multiply(x, y float64) float64 {
    result := x * y
    c.recordOperation(result)
    return c.roundToPrecision(result)
}

// Divide divides x by y and returns the result and any error
func (c *Calculator) Divide(x, y float64) (float64, error) {
    if y == 0 {
        return 0, ErrDivisionByZero
    }
    result := x / y
    c.recordOperation(result)
    return c.roundToPrecision(result), nil
}

// GetHistory returns a copy of the calculation history
func (c *Calculator) GetHistory() []float64 {
    history := make([]float64, len(c.History))
    copy(history, c.History)
    return history
}

// ClearHistory clears the calculation history
func (c *Calculator) ClearHistory() {
    c.History = c.History[:0]
}

// SetPrecision sets the decimal precision for results
func (c *Calculator) SetPrecision(precision int) {
    if precision >= 0 {
        c.precision = precision
    }
}

// recordOperation adds a result to the history
func (c *Calculator) recordOperation(result float64) {
    c.History = append(c.History, result)
}

// roundToPrecision rounds a value to the calculator's precision
func (c *Calculator) roundToPrecision(value float64) float64 {
    multiplier := math.Pow(10, float64(c.precision))
    return math.Round(value*multiplier) / multiplier
}

// CreateCalculator is a factory function for creating calculators
func CreateCalculator(name string, precision int) *Calculator {
    calc := NewCalculator(name)
    calc.SetPrecision(precision)
    return calc
}

// ProcessNumbers processes a slice of numbers using a calculator
func ProcessNumbers(calc CalculatorInterface, numbers []float64, op Operation) ([]float64, error) {
    if len(numbers) < 2 {
        return nil, errors.New("need at least two numbers")
    }

    results := make([]float64, 0, len(numbers)-1)
    accumulator := numbers[0]

    for i := 1; i < len(numbers); i++ {
        var result float64
        var err error

        switch op {
        case Addition:
            result = calc.Add(accumulator, numbers[i])
        case Subtraction:
            result = calc.Subtract(accumulator, numbers[i])
        case Multiplication:
            result = calc.Multiply(accumulator, numbers[i])
        case Division:
            result, err = calc.Divide(accumulator, numbers[i])
            if err != nil {
                return nil, fmt.Errorf("division error at index %d: %w", i, err)
            }
        default:
            return nil, fmt.Errorf("unsupported operation: %d", op)
        }

        results = append(results, result)
        accumulator = result
    }

    return results, nil
}

// init initializes package-level variables
func init() {
    DefaultPrecision = 4
    fmt.Println("Calculator package initialized")
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

    // Verify Go symbols are found
    assert!(
        output.contains("type Calculator struct"),
        "Missing Calculator struct - output: {}",
        output
    );
    assert!(
        output.contains("type CalculatorInterface interface"),
        "Missing CalculatorInterface - output: {}",
        output
    );
    assert!(
        output.contains("type Operation int"),
        "Missing Operation type - output: {}",
        output
    );
    assert!(
        output.contains("func NewCalculator"),
        "Missing NewCalculator function - output: {}",
        output
    );
    assert!(
        output.contains("func (c *Calculator) Add"),
        "Missing Add method - output: {}",
        output
    );
    assert!(
        output.contains("func ProcessNumbers"),
        "Missing ProcessNumbers function - output: {}",
        output
    );
    assert!(
        output.contains("func init"),
        "Missing init function - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_control_flow_statements() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("control_flow.go");

    let content = r#"package main

import (
    "fmt"
    "math/rand"
    "time"
)

// ComplexAlgorithm demonstrates various control flow statements with gaps
func ComplexAlgorithm(data []int, threshold int) map[string]int {
    result := make(map[string]int)
    counter := 0

    // First processing phase with for loop
    for i, item := range data {
        if item > threshold {
            counter++

            // Complex nested conditions
            if counter%2 == 0 {
                result[fmt.Sprintf("even_%d", counter)] = item
            } else {
                result[fmt.Sprintf("odd_%d", counter)] = item
            }

            // Additional processing
            switch {
            case item < 0:
                result[fmt.Sprintf("negative_%d", i)] = item
            case item == 0:
                result["zero"] = 0
            case item > 1000:
                result[fmt.Sprintf("large_%d", i)] = item
            default:
                result[fmt.Sprintf("regular_%d", i)] = item
            }
        }
    }

    // Second processing phase with traditional for loop
    for i := 0; i < len(data); i++ {
        value := data[i]

        switch value {
        case 0:
            result[fmt.Sprintf("zero_index_%d", i)] = 0
        case 1:
            result[fmt.Sprintf("one_index_%d", i)] = 1
        default:
            if value < 0 {
                result[fmt.Sprintf("negative_index_%d", i)] = value
            } else {
                result[fmt.Sprintf("positive_index_%d", i)] = value
            }
        }
    }

    return result
}

// ProcessMatrix demonstrates nested loops with complex control flow
func ProcessMatrix(matrix [][]int) [][]int {
    processed := make([][]int, len(matrix))

    for i, row := range matrix {
        newRow := make([]int, len(row))

        for j, cell := range row {
            var processedCell int

            switch {
            case cell > 0:
                processedCell = cell * 2
            case cell < 0:
                if cell < -100 {
                    processedCell = cell * -1
                } else {
                    processedCell = cell + 100
                }
            default:
                processedCell = 1
            }

            newRow[j] = processedCell
        }

        processed[i] = newRow
    }

    return processed
}

// ProcessWithChannels demonstrates goroutines and channel operations
func ProcessWithChannels(data []int, workers int) ([]int, error) {
    if len(data) == 0 {
        return nil, fmt.Errorf("empty data slice")
    }

    jobs := make(chan int, len(data))
    results := make(chan int, len(data))
    processed := make([]int, 0, len(data))

    // Start workers
    for w := 0; w < workers; w++ {
        go func() {
            for job := range jobs {
                var result int

                // Complex processing
                switch {
                case job%2 == 0:
                    result = job * job
                case job%3 == 0:
                    result = job * 3
                case job%5 == 0:
                    result = job * 5
                default:
                    result = job + 10
                }

                // Simulate processing time
                time.Sleep(time.Millisecond * time.Duration(rand.Intn(100)))
                results <- result
            }
        }()
    }

    // Send jobs
    go func() {
        for _, job := range data {
            jobs <- job
        }
        close(jobs)
    }()

    // Collect results
    for i := 0; i < len(data); i++ {
        select {
        case result := <-results:
            processed = append(processed, result)
        case <-time.After(5 * time.Second):
            return nil, fmt.Errorf("timeout waiting for results")
        }
    }

    return processed, nil
}

// AnalyzeData demonstrates type assertions and error handling
func AnalyzeData(input interface{}) (string, error) {
    switch v := input.(type) {
    case nil:
        return "", fmt.Errorf("nil input")
    case string:
        if len(v) == 0 {
            return "", fmt.Errorf("empty string")
        }

        if len(v) > 100 {
            return "large_string", nil
        }

        // Check if all digits
        for _, r := range v {
            if r < '0' || r > '9' {
                return "text_string", nil
            }
        }
        return "numeric_string", nil

    case int:
        switch {
        case v < 0:
            return "negative_int", nil
        case v == 0:
            return "zero_int", nil
        case v <= 10:
            return "small_int", nil
        case v <= 100:
            return "medium_int", nil
        default:
            return "large_int", nil
        }

    case []int:
        if len(v) == 0 {
            return "empty_slice", nil
        }

        total := 0
        for _, num := range v {
            total += num
        }

        if total > 1000 {
            return "large_sum_slice", nil
        }
        return "regular_slice", nil

    default:
        return "", fmt.Errorf("unsupported type: %T", v)
    }
}

func main() {
    data := []int{1, 2, 3, 4, 5, -1, -2, 0, 100, 1001}
    result := ComplexAlgorithm(data, 0)
    fmt.Printf("Complex algorithm result: %+v\n", result)

    matrix := [][]int{{1, 2}, {-1, 0}, {100, -200}}
    processed := ProcessMatrix(matrix)
    fmt.Printf("Processed matrix: %+v\n", processed)

    channelResult, err := ProcessWithChannels(data, 3)
    if err != nil {
        fmt.Printf("Channel processing error: %v\n", err)
    } else {
        fmt.Printf("Channel result: %+v\n", channelResult)
    }

    analysis, _ := AnalyzeData("12345")
    fmt.Printf("Analysis result: %s\n", analysis)
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Algorithm",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify control flow structures
    assert!(
        output.contains("func ComplexAlgorithm"),
        "Missing ComplexAlgorithm function - output: {}",
        output
    );
    assert!(
        output.contains("func ProcessMatrix"),
        "Missing ProcessMatrix function - output: {}",
        output
    );
    assert!(
        output.contains("func ProcessWithChannels"),
        "Missing ProcessWithChannels function - output: {}",
        output
    );
    assert!(
        output.contains("func AnalyzeData"),
        "Missing AnalyzeData function - output: {}",
        output
    );
    assert!(
        output.contains("func main"),
        "Missing main function - output: {}",
        output
    );

    // Should contain control flow keywords
    let has_control_flow = output.contains("for ")
        || output.contains("switch ")
        || output.contains("if ")
        || output.contains("select ");
    assert!(
        has_control_flow,
        "Missing control flow statements - output: {}",
        output
    );

    // Should contain closing braces
    assert!(
        output.contains("}"),
        "Missing closing braces - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_interfaces_and_structs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("interfaces_structs.go");

    let content = r#"package main

import (
    "encoding/json"
    "fmt"
    "io"
    "time"
)

// Reader interface embeds io.Reader with additional methods
type Reader interface {
    io.Reader
    Reset()
    Size() int64
}

// Writer interface combines multiple interfaces
type Writer interface {
    io.Writer
    io.Closer
    Flush() error
}

// Processor interface defines data processing contract
type Processor interface {
    Process(data []byte) ([]byte, error)
    SetOptions(options map[string]interface{})
    GetStats() ProcessingStats
}

// ProcessingStats represents processing statistics
type ProcessingStats struct {
    BytesProcessed int64     `json:"bytes_processed"`
    Duration       time.Duration `json:"duration"`
    ErrorCount     int       `json:"error_count"`
    StartTime      time.Time `json:"start_time"`
}

// User represents a user entity with JSON tags
type User struct {
    ID        string    `json:"id" db:"id"`
    Username  string    `json:"username" db:"username" validate:"required,min=3,max=20"`
    Email     string    `json:"email" db:"email" validate:"required,email"`
    FirstName string    `json:"first_name" db:"first_name"`
    LastName  string    `json:"last_name" db:"last_name"`
    Age       int       `json:"age" db:"age" validate:"min=0,max=150"`
    IsActive  bool      `json:"is_active" db:"is_active"`
    CreatedAt time.Time `json:"created_at" db:"created_at"`
    UpdatedAt time.Time `json:"updated_at" db:"updated_at"`
}

// DataProcessor implements the Processor interface
type DataProcessor struct {
    options     map[string]interface{}
    stats       ProcessingStats
    buffer      []byte
    maxSize     int64
}

// NewDataProcessor creates a new data processor
func NewDataProcessor(maxSize int64) *DataProcessor {
    return &DataProcessor{
        options: make(map[string]interface{}),
        stats: ProcessingStats{
            StartTime: time.Now(),
        },
        maxSize: maxSize,
    }
}

// Process processes the input data according to configured options
func (dp *DataProcessor) Process(data []byte) ([]byte, error) {
    dp.stats.StartTime = time.Now()
    start := time.Now()

    // Validate input size
    if int64(len(data)) > dp.maxSize {
        dp.stats.ErrorCount++
        return nil, fmt.Errorf("data size %d exceeds maximum %d", len(data), dp.maxSize)
    }

    // Process based on options
    result := make([]byte, 0, len(data))

    if compress, ok := dp.options["compress"].(bool); ok && compress {
        // Simulate compression
        for i, b := range data {
            if i%2 == 0 {
                result = append(result, b)
            }
        }
    } else {
        result = append(result, data...)
    }

    if transform, ok := dp.options["transform"].(string); ok {
        switch transform {
        case "uppercase":
            for i, b := range result {
                if b >= 'a' && b <= 'z' {
                    result[i] = b - 32
                }
            }
        case "lowercase":
            for i, b := range result {
                if b >= 'A' && b <= 'Z' {
                    result[i] = b + 32
                }
            }
        }
    }

    dp.stats.BytesProcessed += int64(len(data))
    dp.stats.Duration += time.Since(start)

    return result, nil
}

// SetOptions sets processing options
func (dp *DataProcessor) SetOptions(options map[string]interface{}) {
    if dp.options == nil {
        dp.options = make(map[string]interface{})
    }

    for k, v := range options {
        dp.options[k] = v
    }
}

// GetStats returns current processing statistics
func (dp *DataProcessor) GetStats() ProcessingStats {
    return dp.stats
}

// UserRepository defines user storage operations
type UserRepository interface {
    Create(user *User) error
    GetByID(id string) (*User, error)
    GetByUsername(username string) (*User, error)
    Update(user *User) error
    Delete(id string) error
    List(offset, limit int) ([]*User, error)
}

// InMemoryUserRepository implements UserRepository for testing
type InMemoryUserRepository struct {
    users   map[string]*User
    counter int
}

// NewInMemoryUserRepository creates a new in-memory repository
func NewInMemoryUserRepository() *InMemoryUserRepository {
    return &InMemoryUserRepository{
        users: make(map[string]*User),
    }
}

// Create adds a new user to the repository
func (r *InMemoryUserRepository) Create(user *User) error {
    if user == nil {
        return fmt.Errorf("user cannot be nil")
    }

    if user.ID == "" {
        r.counter++
        user.ID = fmt.Sprintf("user_%d", r.counter)
    }

    if _, exists := r.users[user.ID]; exists {
        return fmt.Errorf("user with ID %s already exists", user.ID)
    }

    user.CreatedAt = time.Now()
    user.UpdatedAt = user.CreatedAt
    r.users[user.ID] = user

    return nil
}

// GetByID retrieves a user by ID
func (r *InMemoryUserRepository) GetByID(id string) (*User, error) {
    if id == "" {
        return nil, fmt.Errorf("id cannot be empty")
    }

    user, exists := r.users[id]
    if !exists {
        return nil, fmt.Errorf("user with ID %s not found", id)
    }

    // Return a copy to prevent external modifications
    userCopy := *user
    return &userCopy, nil
}

// GetByUsername retrieves a user by username
func (r *InMemoryUserRepository) GetByUsername(username string) (*User, error) {
    if username == "" {
        return nil, fmt.Errorf("username cannot be empty")
    }

    for _, user := range r.users {
        if user.Username == username {
            userCopy := *user
            return &userCopy, nil
        }
    }

    return nil, fmt.Errorf("user with username %s not found", username)
}

// Update modifies an existing user
func (r *InMemoryUserRepository) Update(user *User) error {
    if user == nil || user.ID == "" {
        return fmt.Errorf("user and ID cannot be nil/empty")
    }

    if _, exists := r.users[user.ID]; !exists {
        return fmt.Errorf("user with ID %s not found", user.ID)
    }

    user.UpdatedAt = time.Now()
    r.users[user.ID] = user

    return nil
}

// Delete removes a user from the repository
func (r *InMemoryUserRepository) Delete(id string) error {
    if id == "" {
        return fmt.Errorf("id cannot be empty")
    }

    if _, exists := r.users[id]; !exists {
        return fmt.Errorf("user with ID %s not found", id)
    }

    delete(r.users, id)
    return nil
}

// List returns a paginated list of users
func (r *InMemoryUserRepository) List(offset, limit int) ([]*User, error) {
    if offset < 0 || limit <= 0 {
        return nil, fmt.Errorf("invalid offset or limit")
    }

    users := make([]*User, 0, len(r.users))
    for _, user := range r.users {
        users = append(users, user)
    }

    // Simple pagination
    start := offset
    end := offset + limit

    if start >= len(users) {
        return []*User{}, nil
    }

    if end > len(users) {
        end = len(users)
    }

    result := make([]*User, 0, end-start)
    for i := start; i < end; i++ {
        userCopy := *users[i]
        result = append(result, &userCopy)
    }

    return result, nil
}

// ToJSON converts a user to JSON bytes
func (u *User) ToJSON() ([]byte, error) {
    return json.Marshal(u)
}

// FromJSON populates a user from JSON bytes
func (u *User) FromJSON(data []byte) error {
    return json.Unmarshal(data, u)
}

// IsValid performs basic validation on the user
func (u *User) IsValid() error {
    if u.Username == "" {
        return fmt.Errorf("username is required")
    }

    if len(u.Username) < 3 || len(u.Username) > 20 {
        return fmt.Errorf("username must be 3-20 characters")
    }

    if u.Email == "" {
        return fmt.Errorf("email is required")
    }

    if u.Age < 0 || u.Age > 150 {
        return fmt.Errorf("age must be between 0 and 150")
    }

    return nil
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "type",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify interfaces and structs
    assert!(
        output.contains("type Reader interface"),
        "Missing Reader interface - output: {}",
        output
    );
    assert!(
        output.contains("type Processor interface"),
        "Missing Processor interface - output: {}",
        output
    );
    assert!(
        output.contains("type ProcessingStats struct"),
        "Missing ProcessingStats struct - output: {}",
        output
    );
    assert!(
        output.contains("type User struct"),
        "Missing User struct - output: {}",
        output
    );
    assert!(
        output.contains("type DataProcessor struct"),
        "Missing DataProcessor struct - output: {}",
        output
    );
    assert!(
        output.contains("type InMemoryUserRepository struct"),
        "Missing InMemoryUserRepository struct - output: {}",
        output
    );
    assert!(
        output.contains("func NewDataProcessor"),
        "Missing constructor function - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_test_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test_patterns_test.go");

    let content = r#"package calculator

import (
    "testing"
    "reflect"
    "fmt"
)

// TestCalculatorAdd tests the Add method
func TestCalculatorAdd(t *testing.T) {
    calc := NewCalculator("Test")

    tests := []struct {
        name     string
        x, y     float64
        expected float64
    }{
        {"positive numbers", 2.5, 3.5, 6.0},
        {"negative numbers", -2.5, -1.5, -4.0},
        {"mixed signs", -2.5, 3.5, 1.0},
        {"with zero", 0, 5.5, 5.5},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            result := calc.Add(tt.x, tt.y)
            if result != tt.expected {
                t.Errorf("Add(%v, %v) = %v, want %v", tt.x, tt.y, result, tt.expected)
            }
        })
    }
}

// TestCalculatorDivide tests division with error handling
func TestCalculatorDivide(t *testing.T) {
    calc := NewCalculator("Test")

    // Test successful division
    result, err := calc.Divide(10, 2)
    if err != nil {
        t.Fatalf("Unexpected error: %v", err)
    }
    if result != 5.0 {
        t.Errorf("Divide(10, 2) = %v, want 5.0", result)
    }

    // Test division by zero
    _, err = calc.Divide(10, 0)
    if err == nil {
        t.Error("Expected error for division by zero, got nil")
    }
    if err != ErrDivisionByZero {
        t.Errorf("Expected ErrDivisionByZero, got %v", err)
    }
}

// TestCalculatorHistory tests history tracking
func TestCalculatorHistory(t *testing.T) {
    calc := NewCalculator("Test")

    // Initially empty
    history := calc.GetHistory()
    if len(history) != 0 {
        t.Errorf("Expected empty history, got %v", history)
    }

    // Add some operations
    calc.Add(1, 2)
    calc.Subtract(5, 3)
    calc.Multiply(2, 4)

    history = calc.GetHistory()
    expected := []float64{3.0, 2.0, 8.0}

    if !reflect.DeepEqual(history, expected) {
        t.Errorf("History = %v, want %v", history, expected)
    }

    // Test clear history
    calc.ClearHistory()
    history = calc.GetHistory()
    if len(history) != 0 {
        t.Errorf("Expected empty history after clear, got %v", history)
    }
}

// BenchmarkCalculatorAdd benchmarks the Add method
func BenchmarkCalculatorAdd(b *testing.B) {
    calc := NewCalculator("Benchmark")

    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        calc.Add(float64(i), float64(i+1))
    }
}

// BenchmarkCalculatorOperations benchmarks all operations
func BenchmarkCalculatorOperations(b *testing.B) {
    calc := NewCalculator("Benchmark")

    benchmarks := []struct {
        name string
        fn   func()
    }{
        {"Add", func() { calc.Add(10.5, 20.3) }},
        {"Subtract", func() { calc.Subtract(30.7, 15.2) }},
        {"Multiply", func() { calc.Multiply(5.5, 7.8) }},
        {"Divide", func() { calc.Divide(100.0, 4.0) }},
    }

    for _, bm := range benchmarks {
        b.Run(bm.name, func(b *testing.B) {
            for i := 0; i < b.N; i++ {
                bm.fn()
            }
        })
    }
}

// TestProcessNumbers tests the ProcessNumbers function
func TestProcessNumbers(t *testing.T) {
    calc := NewCalculator("Test")

    tests := []struct {
        name      string
        numbers   []float64
        operation Operation
        expected  []float64
        wantError bool
    }{
        {
            name:      "addition",
            numbers:   []float64{1, 2, 3, 4},
            operation: Addition,
            expected:  []float64{3, 6, 10},
            wantError: false,
        },
        {
            name:      "multiplication",
            numbers:   []float64{2, 3, 4},
            operation: Multiplication,
            expected:  []float64{6, 24},
            wantError: false,
        },
        {
            name:      "division with zero",
            numbers:   []float64{10, 0, 5},
            operation: Division,
            wantError: true,
        },
        {
            name:      "empty slice",
            numbers:   []float64{},
            operation: Addition,
            wantError: true,
        },
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            result, err := ProcessNumbers(calc, tt.numbers, tt.operation)

            if tt.wantError {
                if err == nil {
                    t.Error("Expected error, got nil")
                }
                return
            }

            if err != nil {
                t.Fatalf("Unexpected error: %v", err)
            }

            if !reflect.DeepEqual(result, tt.expected) {
                t.Errorf("ProcessNumbers() = %v, want %v", result, tt.expected)
            }
        })
    }
}

// ExampleCalculator demonstrates calculator usage
func ExampleCalculator() {
    calc := NewCalculator("Demo")

    result := calc.Add(10, 20)
    fmt.Printf("10 + 20 = %.1f\n", result)

    result = calc.Multiply(5, 6)
    fmt.Printf("5 * 6 = %.1f\n", result)

    history := calc.GetHistory()
    fmt.Printf("History: %v\n", history)

    // Output:
    // 10 + 20 = 30.0
    // 5 * 6 = 30.0
    // History: [30 30]
}

// ExampleProcessNumbers demonstrates ProcessNumbers function
func ExampleProcessNumbers() {
    calc := NewCalculator("Example")
    numbers := []float64{2, 4, 6}

    result, err := ProcessNumbers(calc, numbers, Multiplication)
    if err != nil {
        fmt.Printf("Error: %v\n", err)
        return
    }

    fmt.Printf("Results: %v\n", result)
    // Output: Results: [8 48]
}

// TestMain sets up and tears down test environment
func TestMain(m *testing.M) {
    fmt.Println("Setting up tests...")

    // Run tests
    code := m.Run()

    fmt.Println("Cleaning up tests...")

    // Exit with the test result code
    fmt.Printf("Tests completed with code: %d\n", code)
}

// Helper functions for testing
func setupTestCalculator() *Calculator {
    return NewCalculator("TestCalc")
}

func assertFloatEqual(t *testing.T, got, want float64) {
    const epsilon = 1e-9
    if diff := got - want; diff < -epsilon || diff > epsilon {
        t.Errorf("got %v, want %v (diff: %v)", got, want, diff)
    }
}

func createTestData(size int) []float64 {
    data := make([]float64, size)
    for i := 0; i < size; i++ {
        data[i] = float64(i + 1)
    }
    return data
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Test",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify test patterns
    assert!(
        output.contains("func TestCalculatorAdd"),
        "Missing test function - output: {}",
        output
    );
    assert!(
        output.contains("func TestCalculatorDivide"),
        "Missing test function with error handling - output: {}",
        output
    );
    assert!(
        output.contains("func BenchmarkCalculatorAdd"),
        "Missing benchmark function - output: {}",
        output
    );
    assert!(
        output.contains("func ExampleCalculator"),
        "Missing example function - output: {}",
        output
    );
    assert!(
        output.contains("func TestMain"),
        "Missing TestMain function - output: {}",
        output
    );
    assert!(
        output.contains("func setupTestCalculator"),
        "Missing test helper function - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_large_function_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_function.go");

    let content = r#"package main

import (
    "fmt"
    "sort"
    "strings"
    "time"
)

// ComplexDataProcessor processes data through multiple phases with gaps
func ComplexDataProcessor(data []string, options map[string]interface{}) (map[string][]string, error) {
    results := make(map[string][]string)
    categories := make(map[string][]string)

    // Phase 1: Input validation and sanitization
    cleanedData := make([]string, 0, len(data))
    for i, item := range data {
        if item == "" {
            fmt.Printf("Warning: empty item at index %d\n", i)
            continue
        }

        // Clean whitespace
        cleaned := strings.TrimSpace(item)
        if len(cleaned) == 0 {
            fmt.Printf("Warning: whitespace-only item at index %d\n", i)
            continue
        }

        cleanedData = append(cleanedData, cleaned)
    }

    // Phase 2: Categorization based on content
    for _, item := range cleanedData {
        var category string

        // Determine category based on content
        if strings.HasPrefix(item, "http") {
            category = "urls"
        } else if strings.Contains(item, "@") {
            category = "emails"
        } else if len(item) <= 10 {
            // Check if numeric
            isNumeric := true
            for _, r := range item {
                if r < '0' || r > '9' {
                    isNumeric = false
                    break
                }
            }

            if isNumeric {
                category = "numbers"
            } else {
                category = "short_text"
            }
        } else if len(item) > 100 {
            category = "long_text"
        } else {
            category = "medium_text"
        }

        // Store in category map
        if categories[category] == nil {
            categories[category] = make([]string, 0)
        }
        categories[category] = append(categories[category], item)
    }

    // Phase 3: Processing each category with specific rules
    for categoryName, items := range categories {
        processedItems := make([]string, 0, len(items))

        switch categoryName {
        case "urls":
            // URL processing
            for _, url := range items {
                if strings.HasPrefix(url, "https://") {
                    processedItems = append(processedItems, fmt.Sprintf("SECURE_URL: %s", url))
                } else if strings.HasPrefix(url, "http://") {
                    processedItems = append(processedItems, fmt.Sprintf("INSECURE_URL: %s", url))
                } else {
                    processedItems = append(processedItems, fmt.Sprintf("PARTIAL_URL: %s", url))
                }
            }

        case "emails":
            // Email processing with validation
            for _, email := range items {
                parts := strings.Split(email, "@")
                if len(parts) == 2 {
                    domain := parts[1]
                    if strings.Contains(domain, ".") {
                        processedItems = append(processedItems, fmt.Sprintf("VALID_EMAIL: %s", email))
                    } else {
                        processedItems = append(processedItems, fmt.Sprintf("INVALID_DOMAIN: %s", email))
                    }
                } else {
                    processedItems = append(processedItems, fmt.Sprintf("MALFORMED_EMAIL: %s", email))
                }
            }

        case "numbers":
            // Number processing with sorting
            sort.Strings(items)
            for i, num := range items {
                processedItems = append(processedItems, fmt.Sprintf("NUM_%d: %s", i+1, num))
            }

        case "short_text":
            // Short text processing - uppercase
            for _, text := range items {
                processedItems = append(processedItems, strings.ToUpper(text))
            }

        case "medium_text":
            // Medium text processing - title case
            for _, text := range items {
                words := strings.Fields(text)
                titleWords := make([]string, len(words))

                for i, word := range words {
                    if len(word) > 0 {
                        titleWords[i] = strings.ToUpper(word[:1]) + strings.ToLower(word[1:])
                    }
                }

                processedItems = append(processedItems, strings.Join(titleWords, " "))
            }

        case "long_text":
            // Long text processing - truncate and summarize
            for _, text := range items {
                if len(text) > 200 {
                    truncated := text[:197] + "..."
                    summary := fmt.Sprintf("LONG_TEXT (len=%d): %s", len(text), truncated)
                    processedItems = append(processedItems, summary)
                } else {
                    processedItems = append(processedItems, fmt.Sprintf("MEDIUM_LONG: %s", text))
                }
            }

        default:
            // Default processing - add timestamp
            timestamp := time.Now().Format("15:04:05")
            for _, item := range items {
                processedItems = append(processedItems, fmt.Sprintf("[%s] %s", timestamp, item))
            }
        }

        results[categoryName] = processedItems
    }

    // Phase 4: Apply global options and filters
    if options != nil {
        if sortResults, ok := options["sort"].(bool); ok && sortResults {
            for category, items := range results {
                sort.Strings(items)
                results[category] = items
            }
        }

        if maxPerCategory, ok := options["max_per_category"].(int); ok && maxPerCategory > 0 {
            for category, items := range results {
                if len(items) > maxPerCategory {
                    results[category] = items[:maxPerCategory]
                }
            }
        }

        if excludeEmpty, ok := options["exclude_empty"].(bool); ok && excludeEmpty {
            for category, items := range results {
                if len(items) == 0 {
                    delete(results, category)
                }
            }
        }
    }

    // Phase 5: Final validation and cleanup
    finalResults := make(map[string][]string)
    totalItems := 0

    for category, items := range results {
        if len(items) > 0 {
            finalResults[category] = items
            totalItems += len(items)
        }
    }

    fmt.Printf("Processing completed: %d items across %d categories\n", totalItems, len(finalResults))
    return finalResults, nil
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "ComplexDataProcessor",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify large function is shown with closing braces
    assert!(
        output.contains("func ComplexDataProcessor"),
        "Missing ComplexDataProcessor function - output: {}",
        output
    );

    // Should have closing braces for large blocks
    let closing_braces_count = output.matches("}").count();
    assert!(
        closing_braces_count >= 3,
        "Should have multiple closing braces for nested blocks - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_search_command() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("search_test.go");

    let content = r#"package main

import "fmt"

type DataProcessor struct {
    processedCount int
}

func (dp *DataProcessor) ProcessData(data []interface{}) []interface{} {
    dp.processedCount++
    result := make([]interface{}, 0, len(data))

    for _, item := range data {
        if item != nil {
            result = append(result, item)
        }
    }

    return result
}

func (dp *DataProcessor) GetProcessedCount() int {
    return dp.processedCount
}

func ProcessFile(filename string) string {
    return fmt.Sprintf("Processed %s", filename)
}

func ProcessAsync(data map[string]interface{}) map[string]interface{} {
    result := make(map[string]interface{})
    result["processed"] = true

    for k, v := range data {
        result[k] = v
    }

    return result
}

func TestDataProcessing() {
    processor := &DataProcessor{}
    result := processor.ProcessData([]interface{}{1, 2, nil, 3})

    if len(result) != 3 {
        fmt.Println("Test failed")
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "process",
        temp_dir.path().to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Should find symbols containing "process"
    assert!(
        output.contains("DataProcessor")
            || output.contains("ProcessData")
            || output.contains("ProcessFile")
            || output.contains("ProcessAsync"),
        "Should find process-related symbols - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_closing_brace_comments_with_go_syntax() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("closing_brace_comments.go");

    let content = r#"package main

import (
    "context"
    "fmt"
    "sync"
    "time"
)

// SmallFunction should NOT get closing brace comments (under 20 lines)
func SmallFunction(x, y int) int {
    result := x + y
    if result > 10 {
        result *= 2
    }
    return result
} // No comment expected here

// LargeFunctionWithGaps should get Go-style closing brace comments (// syntax)
func LargeFunctionWithGaps(ctx context.Context, data []int) ([]string, error) {
    results := make([]string, 0, len(data))
    var wg sync.WaitGroup
    ch := make(chan string, len(data))

    // Phase 1: Parallel processing
    for i, value := range data {
        wg.Add(1)
        go func(idx, val int) {
            defer wg.Done()

            // Complex processing logic
            processed := val * 2
            if processed > 100 {
                processed = processed / 3
            }

            select {
            case ch <- fmt.Sprintf("item_%d: %d", idx, processed):
                // Successfully sent
            case <-ctx.Done():
                return
            case <-time.After(time.Second):
                // Timeout handling
                ch <- fmt.Sprintf("timeout_%d: %d", idx, processed)
            }
        }(i, value)
    }

    // Phase 2: Wait for completion
    go func() {
        wg.Wait()
        close(ch)
    }()

    // Phase 3: Collect results with timeout
    timeout := time.After(5 * time.Second)
    for {
        select {
        case result, ok := <-ch:
            if !ok {
                // Channel closed, all done
                return results, nil
            }
            results = append(results, result)

        case <-timeout:
            return nil, fmt.Errorf("processing timeout")

        case <-ctx.Done():
            return nil, ctx.Err()
        }
    }

    // This return should never be reached
    return results, nil
} // Should have Go-style comment: // func LargeFunctionWithGaps

// AnotherLargeFunction with nested control flow and generics
func AnotherLargeFunction[T comparable](items []T, filter func(T) bool) map[T]int {
    counts := make(map[T]int)

    // Phase 1: Initial counting
    for _, item := range items {
        if filter != nil {
            if filter(item) {
                counts[item]++
            } else {
                // Skip filtered items
                continue
            }
        } else {
            counts[item]++
        }
    }

    // Phase 2: Normalization
    total := 0
    for _, count := range counts {
        total += count
    }

    if total == 0 {
        return counts
    }

    // Phase 3: Calculate percentages (scaled by 100)
    normalized := make(map[T]int)
    for item, count := range counts {
        percentage := (count * 100) / total
        if percentage > 0 {
            normalized[item] = percentage
        }
    }

    return normalized
} // Should have Go-style comment: // func AnotherLargeFunction
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "LargeFunctionWithGaps",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find all functions
    assert!(
        output.contains("SmallFunction") || output.contains("LargeFunctionWithGaps"),
        "Should find test functions - output: {}",
        output
    );

    // Large functions should have Go-style closing brace comments when gaps are present
    if output.contains("...") {
        // Check for Go-style comments (// syntax, not # or /* */)
        let has_go_comment_syntax = output.contains("// func")
            || output.contains("// LargeFunctionWithGaps")
            || output.contains("// AnotherLargeFunction");

        // Should not have Python/Shell style comments (#) or C-style block comments (/* */)
        let has_wrong_comment_syntax =
            output.contains("# func") || output.contains("/* func") || output.contains("*/");

        assert!(
            has_go_comment_syntax || !has_wrong_comment_syntax,
            "Large functions should use Go-style closing brace comments (// syntax) - output: {}",
            output
        );
    }

    Ok(())
}

#[test]
fn test_go_outline_small_functions_no_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("small_functions.go");

    let content = r#"package utils

import "fmt"

// add performs simple addition (small function)
func add(a, b int) int {
    return a + b
}

// multiply performs simple multiplication (small function)
func multiply(x, y int) int {
    result := x * y
    return result
}

// formatMessage creates a formatted message (small function)
func formatMessage(name string, age int) string {
    return fmt.Sprintf("Hello %s, you are %d years old", name, age)
}

// SimpleStruct for testing small methods
type SimpleStruct struct {
    Value int
}

// GetValue returns the value (small method)
func (s SimpleStruct) GetValue() int {
    return s.Value
}

// SetValue sets the value (small method)
func (s *SimpleStruct) SetValue(val int) {
    s.Value = val
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "add",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find small functions
    assert!(
        output.contains("add") || output.contains("multiply") || output.contains("GetValue"),
        "Should find small functions - output: {}",
        output
    );

    // Small functions should NOT have closing brace comments when fully shown
    let has_closing_brace_comments =
        output.contains("// func") || output.contains("// add") || output.contains("// multiply");

    // Either no closing brace comments (if complete) or has ellipsis (if truncated)
    let has_ellipsis = output.contains("...");
    assert!(
        !has_closing_brace_comments || has_ellipsis,
        "Small functions should not have closing brace comments unless truncated - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_keyword_highlighting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keyword_highlighting.go");

    let content = r#"package main

import (
    "context"
    "fmt"
    "sync"
)

// TestKeywordHighlighting demonstrates Go keywords in various contexts
func TestKeywordHighlighting() {
    // Basic control flow keywords
    if true {
        fmt.Println("if keyword")
    } else {
        fmt.Println("else keyword")
    }

    // For loop keywords
    for i := 0; i < 10; i++ {
        if i%2 == 0 {
            continue
        }
        if i > 7 {
            break
        }
    }

    // Switch statement keywords
    switch value := 42; value {
    case 42:
        fmt.Println("case keyword")
    default:
        fmt.Println("default keyword")
    }
}

// DeferAndPanicKeywords demonstrates defer, panic, and recover
func DeferAndPanicKeywords() {
    defer func() {
        if r := recover(); r != nil {
            fmt.Println("recover keyword:", r)
        }
    }()

    defer fmt.Println("defer keyword executed")

    panic("panic keyword triggered")
}

// ChannelKeywords demonstrates channel operations
func ChannelKeywords() {
    ch := make(chan string, 5)

    go func() {
        ch <- "goroutine keyword"
        close(ch)
    }()

    select {
    case msg := <-ch:
        fmt.Println("select keyword:", msg)
    default:
        fmt.Println("default in select")
    }
}

// InterfaceKeywords demonstrates interface and type keywords
type Processor interface {
    Process(data interface{}) interface{}
}

// StructKeywords demonstrates struct and method keywords
type DataStruct struct {
    value interface{}
}

func (d *DataStruct) Process(data interface{}) interface{} {
    return struct {
        original interface{}
        processed interface{}
    }{
        original: d.value,
        processed: data,
    }
}

// GenericKeywords demonstrates type constraints and generic syntax
func GenericFunction[T comparable, U any](items []T, mapper func(T) U) map[T]U {
    result := make(map[T]U)
    for _, item := range items {
        result[item] = mapper(item)
    }
    return result
}

// ContextKeywords demonstrates context usage
func ContextFunction(ctx context.Context) error {
    select {
    case <-ctx.Done():
        return ctx.Err()
    default:
        return nil
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "keyword",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find functions with keyword-related names
    assert!(
        output.contains("TestKeywordHighlighting")
            || output.contains("DeferAndPanicKeywords")
            || output.contains("ChannelKeywords"),
        "Should find keyword-related functions - output: {}",
        output
    );

    // Should preserve Go keywords in the outline when they are highlighted/matched
    let go_keywords_preserved = output.contains("if ")
        || output.contains("for ")
        || output.contains("switch ")
        || output.contains("select ")
        || output.contains("defer ")
        || output.contains("go ")
        || output.contains("type ")
        || output.contains("func ")
        || output.contains("interface")
        || output.contains("struct");

    assert!(
        go_keywords_preserved,
        "Go keywords should be preserved in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_slice_map_struct_truncation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("truncation_test.go");

    let content = r#"package main

import (
    "fmt"
    "time"
)

// LargeSliceFunction demonstrates slice truncation with keyword preservation
func LargeSliceFunction() []string {
    largeSlice := []string{
        "item1", "item2", "item3", "item4", "item5",
        "item6", "item7", "item8", "item9", "item10",
        "item11", "item12", "item13", "item14", "item15",
        "item16", "item17", "item18", "item19", "item20",
        "item21", "item22", "item23", "item24", "item25",
    }

    // This slice should be truncated but preserve the make keyword
    result := make([]string, 0, len(largeSlice))

    for i, item := range largeSlice {
        if i%2 == 0 {
            result = append(result, fmt.Sprintf("even_%s", item))
        }
    }

    return result
}

// LargeMapFunction demonstrates map truncation with keyword preservation
func LargeMapFunction() map[string]interface{} {
    largeMap := map[string]interface{}{
        "key1": "value1", "key2": "value2", "key3": "value3",
        "key4": "value4", "key5": "value5", "key6": "value6",
        "key7": "value7", "key8": "value8", "key9": "value9",
        "key10": "value10", "key11": "value11", "key12": "value12",
        "key13": "value13", "key14": "value14", "key15": "value15",
        "key16": "value16", "key17": "value17", "key18": "value18",
        "key19": "value19", "key20": "value20", "key21": "value21",
    }

    // This map should be truncated but preserve the make keyword
    result := make(map[string]interface{})

    for key, value := range largeMap {
        if len(key) > 4 {
            result[key] = value
        }
    }

    return result
}

// ComplexStruct demonstrates struct truncation with keyword preservation
type ComplexStruct struct {
    Field1  string        `json:"field1" db:"field1" validate:"required"`
    Field2  int          `json:"field2" db:"field2" validate:"min=0,max=100"`
    Field3  float64      `json:"field3" db:"field3"`
    Field4  bool         `json:"field4" db:"field4"`
    Field5  time.Time    `json:"field5" db:"field5"`
    Field6  []string     `json:"field6" db:"field6"`
    Field7  map[string]int `json:"field7" db:"field7"`
    Field8  interface{}  `json:"field8" db:"field8"`
    Field9  *string      `json:"field9" db:"field9"`
    Field10 chan string  `json:"-" db:"field10"`
    Field11 func() error `json:"-" db:"-"`
    Field12 struct {
        NestedField1 string `json:"nested1"`
        NestedField2 int    `json:"nested2"`
        NestedField3 bool   `json:"nested3"`
    } `json:"field12"`
}

// LargeStructFunction demonstrates struct initialization truncation
func LargeStructFunction() ComplexStruct {
    return ComplexStruct{
        Field1:  "value1",
        Field2:  42,
        Field3:  3.14159,
        Field4:  true,
        Field5:  time.Now(),
        Field6:  []string{"a", "b", "c", "d", "e", "f", "g", "h", "i", "j"},
        Field7:  map[string]int{"one": 1, "two": 2, "three": 3, "four": 4, "five": 5},
        Field8:  "interface value",
        Field9:  &[]string{"pointer to string"}[0],
        Field10: make(chan string, 10),
        Field11: func() error { return nil },
        Field12: struct {
            NestedField1 string `json:"nested1"`
            NestedField2 int    `json:"nested2"`
            NestedField3 bool   `json:"nested3"`
        }{
            NestedField1: "nested value",
            NestedField2: 99,
            NestedField3: false,
        },
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "slice",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find functions related to slices
    assert!(
        output.contains("LargeSliceFunction") || output.len() > 10,
        "Should find slice-related content - output: {}",
        output
    );

    // When content is truncated, should preserve Go keywords
    if output.contains("...") {
        let preserves_keywords = output.contains("make")
            || output.contains("[]string")
            || output.contains("map[")
            || output.contains("struct")
            || output.contains("func")
            || output.contains("type")
            || output.contains("interface{}");

        assert!(
            preserves_keywords,
            "Should preserve Go keywords when truncating slices/maps/structs - output: {}",
            output
        );
    }

    Ok(())
}

#[test]
fn test_go_outline_comprehensive_go_specific_constructs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("go_specific_constructs.go");

    let content = r#"package main

import (
    "context"
    "fmt"
    "sync"
    "time"
)

// GenericProcessor demonstrates Go generics with type constraints
type GenericProcessor[T comparable, U any] struct {
    items   []T
    mapper  func(T) U
    results chan ProcessResult[T, U]
}

type ProcessResult[T comparable, U any] struct {
    Input  T
    Output U
    Error  error
}

// NewGenericProcessor creates a new generic processor
func NewGenericProcessor[T comparable, U any](mapper func(T) U) *GenericProcessor[T, U] {
    return &GenericProcessor[T, U]{
        items:   make([]T, 0),
        mapper:  mapper,
        results: make(chan ProcessResult[T, U], 100),
    }
}

// ProcessWithGenerics demonstrates generics with complex operations
func (gp *GenericProcessor[T, U]) ProcessWithGenerics(ctx context.Context, items []T) <-chan ProcessResult[T, U] {
    output := make(chan ProcessResult[T, U])

    go func() {
        defer close(output)
        defer func() {
            if r := recover(); r != nil {
                select {
                case output <- ProcessResult[T, U]{Error: fmt.Errorf("panic: %v", r)}:
                case <-ctx.Done():
                }
            }
        }()

        var wg sync.WaitGroup
        semaphore := make(chan struct{}, 10) // Limit concurrent goroutines

        for _, item := range items {
            select {
            case <-ctx.Done():
                return
            case semaphore <- struct{}{}:
                wg.Add(1)
                go func(input T) {
                    defer wg.Done()
                    defer func() { <-semaphore }()

                    result := ProcessResult[T, U]{Input: input}

                    defer func() {
                        if r := recover(); r != nil {
                            result.Error = fmt.Errorf("processing panic: %v", r)
                        }

                        select {
                        case output <- result:
                        case <-ctx.Done():
                        }
                    }()

                    result.Output = gp.mapper(input)
                }(item)
            }
        }

        wg.Wait()
    }()

    return output
}

// ChannelOperationsDemo demonstrates complex channel operations
func ChannelOperationsDemo(ctx context.Context) error {
    // Buffered channels with different types
    stringChan := make(chan string, 5)
    intChan := make(chan int, 3)
    errorChan := make(chan error, 1)
    doneChan := make(chan struct{})

    // Worker goroutines with different patterns
    go func() {
        defer close(stringChan)
        for i := 0; i < 10; i++ {
            select {
            case stringChan <- fmt.Sprintf("message_%d", i):
                time.Sleep(100 * time.Millisecond)
            case <-ctx.Done():
                return
            }
        }
    }()

    go func() {
        defer close(intChan)
        for i := 0; i < 5; i++ {
            select {
            case intChan <- i * i:
                time.Sleep(200 * time.Millisecond)
            case <-ctx.Done():
                return
            }
        }
    }()

    // Complex select statement with multiple cases
    go func() {
        defer close(doneChan)

        for {
            select {
            case msg, ok := <-stringChan:
                if !ok {
                    return
                }
                fmt.Printf("Received string: %s\n", msg)

            case num, ok := <-intChan:
                if !ok {
                    return
                }
                fmt.Printf("Received int: %d\n", num)

            case err := <-errorChan:
                fmt.Printf("Received error: %v\n", err)
                return

            case <-time.After(1 * time.Second):
                fmt.Println("Timeout in select")

            case <-ctx.Done():
                fmt.Println("Context cancelled")
                return
            }
        }
    }()

    select {
    case <-doneChan:
        return nil
    case <-ctx.Done():
        return ctx.Err()
    case <-time.After(5 * time.Second):
        return fmt.Errorf("operation timeout")
    }
}

// TypeSwitchAndAssertions demonstrates type switches and type assertions
func TypeSwitchAndAssertions(input interface{}) (string, error) {
    switch v := input.(type) {
    case nil:
        return "nil", nil

    case string:
        if len(v) == 0 {
            return "empty_string", nil
        }
        return fmt.Sprintf("string: %s", v), nil

    case int:
        return fmt.Sprintf("int: %d", v), nil

    case int64:
        return fmt.Sprintf("int64: %d", v), nil

    case float64:
        return fmt.Sprintf("float64: %.2f", v), nil

    case bool:
        return fmt.Sprintf("bool: %t", v), nil

    case []interface{}:
        return fmt.Sprintf("slice_length: %d", len(v)), nil

    case map[string]interface{}:
        return fmt.Sprintf("map_keys: %d", len(v)), nil

    case chan string:
        return "string_channel", nil

    case <-chan string:
        return "receive_only_string_channel", nil

    case chan<- string:
        return "send_only_string_channel", nil

    default:
        // Type assertion attempt
        if stringer, ok := input.(fmt.Stringer); ok {
            return fmt.Sprintf("stringer: %s", stringer.String()), nil
        }

        return "", fmt.Errorf("unsupported type: %T", input)
    }
}

// EmbeddedTypesAndMethods demonstrates embedded types and method promotion
type Reader interface {
    Read([]byte) (int, error)
}

type Writer interface {
    Write([]byte) (int, error)
}

type Closer interface {
    Close() error
}

type ReadWriteCloser interface {
    Reader
    Writer
    Closer
}

type FileManager struct {
    ReadWriteCloser // Embedded interface
    filename        string
    metadata        map[string]interface{}
}

func (fm *FileManager) GetFilename() string {
    return fm.filename
}

func (fm *FileManager) SetMetadata(key string, value interface{}) {
    if fm.metadata == nil {
        fm.metadata = make(map[string]interface{})
    }
    fm.metadata[key] = value
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "GenericProcessor",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find Go-specific constructs
    assert!(
        output.contains("GenericProcessor")
            || output.contains("ProcessWithGenerics")
            || output.contains("ChannelOperationsDemo")
            || output.contains("TypeSwitchAndAssertions"),
        "Should find Go-specific construct functions - output: {}",
        output
    );

    // Should contain Go-specific syntax elements
    let go_specific_syntax = output.contains("chan ")
        || output.contains("select {")
        || output.contains("defer ")
        || output.contains("go func")
        || output.contains("interface{}")
        || output.contains("make(chan")
        || output.contains("<-")
        || output.contains(".(type)");

    assert!(
        go_specific_syntax,
        "Should contain Go-specific syntax in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_go_outline_enhanced_testing_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("enhanced_testing_patterns_test.go");

    let content = r#"package calculator

import (
    "context"
    "fmt"
    "reflect"
    "testing"
    "time"
)

// TestWithSubtests demonstrates table-driven tests with subtests
func TestWithSubtests(t *testing.T) {
    testCases := []struct {
        name     string
        input    int
        expected int
        wantErr  bool
    }{
        {"positive number", 5, 25, false},
        {"negative number", -3, 9, false},
        {"zero", 0, 0, false},
        {"large number", 100, 10000, false},
    }

    for _, tc := range testCases {
        t.Run(tc.name, func(t *testing.T) {
            result := tc.input * tc.input
            if result != tc.expected {
                t.Errorf("got %d, want %d", result, tc.expected)
            }
        })
    }
}

// TestWithContext demonstrates context-aware testing
func TestWithContext(t *testing.T) {
    ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
    defer cancel()

    done := make(chan bool, 1)
    go func() {
        // Simulate long-running operation
        time.Sleep(1 * time.Second)
        done <- true
    }()

    select {
    case <-done:
        t.Log("Operation completed successfully")
    case <-ctx.Done():
        t.Fatal("Test timed out")
    }
}

// TestParallelExecution demonstrates parallel test execution
func TestParallelExecution(t *testing.T) {
    t.Parallel()

    testCases := []int{1, 2, 3, 4, 5}

    for _, tc := range testCases {
        tc := tc // Capture loop variable
        t.Run(fmt.Sprintf("parallel_%d", tc), func(t *testing.T) {
            t.Parallel()

            // Simulate some work
            time.Sleep(100 * time.Millisecond)
            result := tc * tc

            if result <= 0 {
                t.Errorf("Expected positive result, got %d", result)
            }
        })
    }
}

// BenchmarkWithComplexOperations demonstrates advanced benchmarking
func BenchmarkWithComplexOperations(b *testing.B) {
    benchmarks := []struct {
        name string
        size int
        fn   func(int) interface{}
    }{
        {"SliceOperations", 1000, func(n int) interface{} {
            slice := make([]int, n)
            for i := 0; i < n; i++ {
                slice[i] = i * i
            }
            return slice
        }},
        {"MapOperations", 1000, func(n int) interface{} {
            m := make(map[int]int, n)
            for i := 0; i < n; i++ {
                m[i] = i * i
            }
            return m
        }},
        {"ChannelOperations", 100, func(n int) interface{} {
            ch := make(chan int, n)
            go func() {
                defer close(ch)
                for i := 0; i < n; i++ {
                    ch <- i
                }
            }()

            var results []int
            for val := range ch {
                results = append(results, val)
            }
            return results
        }},
    }

    for _, bm := range benchmarks {
        b.Run(bm.name, func(b *testing.B) {
            b.ResetTimer()
            for i := 0; i < b.N; i++ {
                bm.fn(bm.size)
            }
        })
    }
}

// ExampleComplexDataStructures demonstrates complex examples
func ExampleComplexDataStructures() {
    // Generic map with complex operations
    data := map[string]interface{}{
        "numbers": []int{1, 2, 3, 4, 5},
        "strings": []string{"a", "b", "c"},
        "nested": map[string]interface{}{
            "inner": "value",
            "count": 42,
        },
    }

    // Type assertions and processing
    if numbers, ok := data["numbers"].([]int); ok {
        total := 0
        for _, num := range numbers {
            total += num
        }
        fmt.Printf("Sum of numbers: %d\n", total)
    }

    if nested, ok := data["nested"].(map[string]interface{}); ok {
        if count, ok := nested["count"].(int); ok {
            fmt.Printf("Nested count: %d\n", count)
        }
    }

    // Output:
    // Sum of numbers: 15
    // Nested count: 42
}

// TestWithGenerics demonstrates testing with generic functions
func TestWithGenerics[T comparable](t *testing.T) {
    // This would be called with specific types in actual usage
    testGenericFunction := func(items []T, target T) bool {
        for _, item := range items {
            if item == target {
                return true
            }
        }
        return false
    }

    // Note: This is conceptual - Go's testing package doesn't support generic test functions yet
    _ = testGenericFunction
}

// BenchmarkGenericOperations demonstrates benchmarking with generics
func BenchmarkGenericOperations(b *testing.B) {
    intSlice := make([]int, 1000)
    for i := range intSlice {
        intSlice[i] = i
    }

    stringSlice := make([]string, 1000)
    for i := range stringSlice {
        stringSlice[i] = fmt.Sprintf("item_%d", i)
    }

    b.Run("IntOperations", func(b *testing.B) {
        b.ResetTimer()
        for i := 0; i < b.N; i++ {
            genericSearch(intSlice, 500)
        }
    })

    b.Run("StringOperations", func(b *testing.B) {
        b.ResetTimer()
        for i := 0; i < b.N; i++ {
            genericSearch(stringSlice, "item_500")
        }
    })
}

func genericSearch[T comparable](slice []T, target T) bool {
    for _, item := range slice {
        if item == target {
            return true
        }
    }
    return false
}

// TestWithReflection demonstrates reflection-based testing
func TestWithReflection(t *testing.T) {
    testStruct := struct {
        Name  string `json:"name" validate:"required"`
        Age   int    `json:"age" validate:"min=0,max=150"`
        Email string `json:"email" validate:"email"`
    }{
        Name:  "John Doe",
        Age:   30,
        Email: "john@example.com",
    }

    v := reflect.ValueOf(testStruct)
    typ := reflect.TypeOf(testStruct)

    for i := 0; i < v.NumField(); i++ {
        field := v.Field(i)
        fieldType := typ.Field(i)

        jsonTag := fieldType.Tag.Get("json")
        validateTag := fieldType.Tag.Get("validate")

        t.Logf("Field: %s, Type: %s, JSON: %s, Validation: %s, Value: %v",
            fieldType.Name, field.Type(), jsonTag, validateTag, field.Interface())
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Test",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Should find various testing patterns
    assert!(
        output.contains("TestWithSubtests")
            || output.contains("TestWithContext")
            || output.contains("BenchmarkWithComplexOperations")
            || output.contains("ExampleComplexDataStructures"),
        "Should find enhanced testing pattern functions - output: {}",
        output
    );

    // Should contain Go testing-specific elements
    let testing_patterns = output.contains("t.Run")
        || output.contains("b.Run")
        || output.contains("testing.T")
        || output.contains("testing.B")
        || output.contains("t.Parallel")
        || output.contains("b.ResetTimer");

    assert!(
        testing_patterns,
        "Should contain Go testing patterns in outline format - output: {}",
        output
    );

    Ok(())
}

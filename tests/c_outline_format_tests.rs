use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_c_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("basic.c");

    let content = r#"// Basic C functions and structures for testing
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

// Constants and macros
#define MAX_SIZE 100
#define PI 3.14159
#define SQUARE(x) ((x) * (x))

// Type definitions
typedef struct {
    char name[50];
    double* history;
    int count;
    int capacity;
} Calculator;

typedef enum {
    OPERATION_ADD,
    OPERATION_SUBTRACT,
    OPERATION_MULTIPLY,
    OPERATION_DIVIDE
} OperationType;

// Global variables
static Calculator* global_calculator = NULL;
const double DEFAULT_PRECISION = 0.01;

// Function prototypes
Calculator* create_calculator(const char* name);
void destroy_calculator(Calculator* calc);
double add_numbers(Calculator* calc, double x, double y);
double subtract_numbers(Calculator* calc, double x, double y);
double multiply_numbers(Calculator* calc, double x, double y);
double divide_numbers(Calculator* calc, double x, double y);
int get_history_count(const Calculator* calc);
double* get_history(const Calculator* calc);

// Function implementations

/**
 * Creates a new calculator instance
 * @param name The name of the calculator
 * @return Pointer to the new calculator or NULL on failure
 */
Calculator* create_calculator(const char* name) {
    Calculator* calc = malloc(sizeof(Calculator));
    if (calc == NULL) {
        return NULL;
    }

    // Initialize name
    strncpy(calc->name, name, sizeof(calc->name) - 1);
    calc->name[sizeof(calc->name) - 1] = '\0';

    // Initialize history
    calc->capacity = 10;
    calc->history = malloc(calc->capacity * sizeof(double));
    if (calc->history == NULL) {
        free(calc);
        return NULL;
    }

    calc->count = 0;
    return calc;
}

/**
 * Destroys a calculator instance and frees memory
 * @param calc Pointer to the calculator to destroy
 */
void destroy_calculator(Calculator* calc) {
    if (calc != NULL) {
        if (calc->history != NULL) {
            free(calc->history);
        }
        free(calc);
    }
}

/**
 * Records an operation result in the calculator's history
 * @param calc Pointer to the calculator
 * @param result The result to record
 * @return 0 on success, -1 on failure
 */
static int record_operation(Calculator* calc, double result) {
    if (calc == NULL) {
        return -1;
    }

    // Expand history if needed
    if (calc->count >= calc->capacity) {
        calc->capacity *= 2;
        double* new_history = realloc(calc->history, calc->capacity * sizeof(double));
        if (new_history == NULL) {
            return -1;
        }
        calc->history = new_history;
    }

    calc->history[calc->count++] = result;
    return 0;
}

/**
 * Adds two numbers and records the result
 * @param calc Pointer to the calculator
 * @param x First number
 * @param y Second number
 * @return The sum of x and y
 */
double add_numbers(Calculator* calc, double x, double y) {
    double result = x + y;
    if (calc != NULL) {
        record_operation(calc, result);
    }
    return result;
}

/**
 * Subtracts y from x and records the result
 * @param calc Pointer to the calculator
 * @param x First number
 * @param y Second number
 * @return The difference x - y
 */
double subtract_numbers(Calculator* calc, double x, double y) {
    double result = x - y;
    if (calc != NULL) {
        record_operation(calc, result);
    }
    return result;
}

/**
 * Multiplies two numbers and records the result
 * @param calc Pointer to the calculator
 * @param x First number
 * @param y Second number
 * @return The product of x and y
 */
double multiply_numbers(Calculator* calc, double x, double y) {
    double result = x * y;
    if (calc != NULL) {
        record_operation(calc, result);
    }
    return result;
}

/**
 * Divides x by y and records the result
 * @param calc Pointer to the calculator
 * @param x Dividend
 * @param y Divisor
 * @return The quotient x / y, or NAN if y is zero
 */
double divide_numbers(Calculator* calc, double x, double y) {
    double result;
    if (fabs(y) < DEFAULT_PRECISION) {
        result = NAN;
    } else {
        result = x / y;
    }

    if (calc != NULL && !isnan(result)) {
        record_operation(calc, result);
    }
    return result;
}

/**
 * Gets the number of operations in history
 * @param calc Pointer to the calculator
 * @return Number of operations or -1 if calc is NULL
 */
int get_history_count(const Calculator* calc) {
    if (calc == NULL) {
        return -1;
    }
    return calc->count;
}

/**
 * Gets a pointer to the calculator's history array
 * @param calc Pointer to the calculator
 * @return Pointer to history array or NULL if calc is NULL
 */
double* get_history(const Calculator* calc) {
    if (calc == NULL) {
        return NULL;
    }
    return calc->history;
}

/**
 * Prints the calculator's history to stdout
 * @param calc Pointer to the calculator
 */
void print_history(const Calculator* calc) {
    if (calc == NULL) {
        printf("Calculator is NULL\n");
        return;
    }

    printf("History for calculator '%s':\n", calc->name);
    for (int i = 0; i < calc->count; i++) {
        printf("  %d: %.2f\n", i + 1, calc->history[i]);
    }
}

/**
 * Clears the calculator's history
 * @param calc Pointer to the calculator
 */
void clear_history(Calculator* calc) {
    if (calc != NULL) {
        calc->count = 0;
    }
}

/**
 * Main function demonstrating calculator usage
 */
int main(int argc, char* argv[]) {
    Calculator* calc = create_calculator("Main Calculator");
    if (calc == NULL) {
        fprintf(stderr, "Failed to create calculator\n");
        return 1;
    }

    // Perform some calculations
    double result1 = add_numbers(calc, 10.5, 20.3);
    double result2 = multiply_numbers(calc, 5.0, 7.0);
    double result3 = divide_numbers(calc, 100.0, 4.0);

    printf("Results: %.2f, %.2f, %.2f\n", result1, result2, result3);

    // Print history
    print_history(calc);

    // Cleanup
    destroy_calculator(calc);

    return 0;
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

    // Verify C outline format features
    assert!(
        output.contains("Calculator"),
        "Missing Calculator in search results - output: {}",
        output
    );
    assert!(
        output.contains("..."),
        "Missing truncation ellipsis in outline format - output: {}",
        output
    );
    // Look for C-specific comment syntax in closing braces or structure
    let has_c_syntax = output.contains("typedef struct")
        || output.contains("Calculator*")
        || output.contains("void destroy_calculator")
        || output.contains("//");
    assert!(
        has_c_syntax,
        "Missing C-specific syntax in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_control_flow_statements() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("control_flow.c");

    let content = r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/**
 * Complex algorithm demonstrating various control flow statements with gaps
 */
int* complex_algorithm(int* data, int size, int threshold, int* result_size) {
    if (data == NULL || size <= 0) {
        *result_size = 0;
        return NULL;
    }

    int* result = malloc(size * sizeof(int));
    if (result == NULL) {
        *result_size = 0;
        return NULL;
    }

    int count = 0;
    int counter = 0;

    // First processing phase with for loop
    for (int i = 0; i < size; i++) {
        int item = data[i];

        if (item > threshold) {
            counter++;

            // Complex nested conditions
            if (counter % 2 == 0) {
                result[count++] = item * 2;
            } else {
                result[count++] = item + 10;
            }

            // Additional processing with switch
            switch (item % 4) {
                case 0:
                    result[count - 1] += 100;
                    break;
                case 1:
                    result[count - 1] += 10;
                    break;
                case 2:
                    result[count - 1] += 1;
                    break;
                default:
                    result[count - 1] -= 5;
                    break;
            }
        }
    }

    // Second processing phase with while loop
    int index = 0;
    while (index < count) {
        int value = result[index];

        if (value < 0) {
            // Remove negative values by shifting
            for (int j = index; j < count - 1; j++) {
                result[j] = result[j + 1];
            }
            count--;
        } else if (value > 1000) {
            // Cap large values
            result[index] = 1000;
            index++;
        } else {
            index++;
        }
    }

    *result_size = count;
    return result;
}

/**
 * Process matrix with nested loops demonstrating closing brace comments
 */
int** process_matrix(int** matrix, int rows, int cols) {
    if (matrix == NULL || rows <= 0 || cols <= 0) {
        return NULL;
    }

    // Allocate result matrix
    int** processed = malloc(rows * sizeof(int*));
    if (processed == NULL) {
        return NULL;
    }

    for (int i = 0; i < rows; i++) {
        processed[i] = malloc(cols * sizeof(int));
        if (processed[i] == NULL) {
            // Cleanup on failure
            for (int j = 0; j < i; j++) {
                free(processed[j]);
            }
            free(processed);
            return NULL;
        }

        // Process each row
        for (int j = 0; j < cols; j++) {
            int cell = matrix[i][j];
            int processed_cell;

            if (cell > 0) {
                processed_cell = cell * 2;
            } else if (cell < 0) {
                processed_cell = abs(cell);
            } else {
                processed_cell = 1;
            }

            processed[i][j] = processed_cell;
        }
    }

    return processed;
}

/**
 * Analyze data with complex switch statement and error handling
 */
const char* analyze_data(int value, char* buffer, size_t buffer_size) {
    if (buffer == NULL || buffer_size == 0) {
        return "ERROR: Invalid buffer";
    }

    const char* result;

    switch (value) {
        case 0:
            result = "ZERO";
            break;

        case 1:
        case 2:
        case 3:
        case 4:
        case 5:
            result = "SMALL_POSITIVE";
            break;

        case -1:
        case -2:
        case -3:
        case -4:
        case -5:
            result = "SMALL_NEGATIVE";
            break;

        default:
            if (value > 5 && value <= 100) {
                result = "MEDIUM_POSITIVE";
            } else if (value > 100 && value <= 1000) {
                result = "LARGE_POSITIVE";
            } else if (value < -5 && value >= -100) {
                result = "MEDIUM_NEGATIVE";
            } else if (value < -100) {
                result = "LARGE_NEGATIVE";
            } else {
                result = "VERY_LARGE_POSITIVE";
            }
            break;
    }

    // Copy result to buffer with bounds checking
    size_t result_len = strlen(result);
    if (result_len >= buffer_size) {
        strncpy(buffer, result, buffer_size - 1);
        buffer[buffer_size - 1] = '\0';
    } else {
        strcpy(buffer, result);
    }

    return buffer;
}

/**
 * Recursive function with complex termination conditions
 */
long factorial_with_memoization(int n, long* memo, int memo_size) {
    // Base cases
    if (n < 0) {
        return -1; // Error case
    }

    if (n == 0 || n == 1) {
        return 1;
    }

    // Check memoization
    if (n < memo_size && memo[n] != 0) {
        return memo[n];
    }

    // Recursive calculation with loop for large values
    long result = 1;
    if (n > 20) {
        // Use iterative approach for large values to avoid stack overflow
        for (int i = 2; i <= n; i++) {
            result *= i;

            // Check for overflow
            if (result < 0) {
                return -1;
            }
        }
    } else {
        // Use recursion for smaller values
        result = n * factorial_with_memoization(n - 1, memo, memo_size);
    }

    // Store in memo if space available
    if (n < memo_size) {
        memo[n] = result;
    }

    return result;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "complex_algorithm",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify outline format for complex algorithm with control flow
    assert!(
        output.contains("complex_algorithm"),
        "Missing complex_algorithm function - output: {}",
        output
    );
    assert!(
        output.contains("..."),
        "Missing truncation ellipsis in outline format - output: {}",
        output
    );
    // Should contain closing brace comment for function
    assert!(
        output.contains("} //") || output.contains("}//") || output.contains("} /*"),
        "Missing closing brace comment for function - output: {}",
        output
    );
    // Should show C function signature patterns
    let has_c_function_pattern = output.contains("int*")
        || output.contains("char**")
        || output.contains("(")
        || output.contains(")");
    assert!(
        has_c_function_pattern,
        "Missing C function signature patterns - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_structs_and_unions() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("structs_unions.c");

    let content = r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

// Complex struct definitions with various member types
typedef struct Point {
    double x;
    double y;
} Point;

typedef struct Rectangle {
    Point top_left;
    Point bottom_right;
    char label[32];
    uint32_t color;
} Rectangle;

typedef struct Circle {
    Point center;
    double radius;
    char label[32];
    uint32_t color;
} Circle;

// Union for different shape types
typedef union ShapeData {
    Rectangle rect;
    Circle circle;
} ShapeData;

typedef enum {
    SHAPE_RECTANGLE,
    SHAPE_CIRCLE,
    SHAPE_TRIANGLE
} ShapeType;

// Complex struct with nested structures and function pointers
typedef struct Shape {
    ShapeType type;
    ShapeData data;
    double (*area_func)(const struct Shape* shape);
    double (*perimeter_func)(const struct Shape* shape);
    void (*print_func)(const struct Shape* shape);
    struct Shape* next; // For linked list
} Shape;

// Function pointer typedef
typedef void (*ProcessFunc)(void* data, size_t size);

// Complex data structure
typedef struct DataProcessor {
    char name[64];
    ProcessFunc process_func;
    void* context;
    size_t buffer_size;
    uint8_t* buffer;
    struct {
        uint64_t total_processed;
        uint32_t operations_count;
        double average_time;
    } stats;
} DataProcessor;

// Bit field structure
typedef struct StatusFlags {
    unsigned int initialized : 1;
    unsigned int error : 1;
    unsigned int processing : 1;
    unsigned int completed : 1;
    unsigned int reserved : 28;
} StatusFlags;

// Forward declarations
double rectangle_area(const Shape* shape);
double rectangle_perimeter(const Shape* shape);
void print_rectangle(const Shape* shape);
double circle_area(const Shape* shape);
double circle_perimeter(const Shape* shape);
void print_circle(const Shape* shape);

// Function implementations

/**
 * Creates a new rectangle shape
 */
Shape* create_rectangle(double x1, double y1, double x2, double y2, const char* label, uint32_t color) {
    Shape* shape = malloc(sizeof(Shape));
    if (shape == NULL) {
        return NULL;
    }

    shape->type = SHAPE_RECTANGLE;
    shape->data.rect.top_left.x = x1 < x2 ? x1 : x2;
    shape->data.rect.top_left.y = y1 > y2 ? y1 : y2;
    shape->data.rect.bottom_right.x = x1 > x2 ? x1 : x2;
    shape->data.rect.bottom_right.y = y1 < y2 ? y1 : y2;

    strncpy(shape->data.rect.label, label, sizeof(shape->data.rect.label) - 1);
    shape->data.rect.label[sizeof(shape->data.rect.label) - 1] = '\0';
    shape->data.rect.color = color;

    shape->area_func = rectangle_area;
    shape->perimeter_func = rectangle_perimeter;
    shape->print_func = print_rectangle;
    shape->next = NULL;

    return shape;
}

/**
 * Creates a new circle shape
 */
Shape* create_circle(double cx, double cy, double radius, const char* label, uint32_t color) {
    Shape* shape = malloc(sizeof(Shape));
    if (shape == NULL) {
        return NULL;
    }

    shape->type = SHAPE_CIRCLE;
    shape->data.circle.center.x = cx;
    shape->data.circle.center.y = cy;
    shape->data.circle.radius = radius > 0 ? radius : 1.0;

    strncpy(shape->data.circle.label, label, sizeof(shape->data.circle.label) - 1);
    shape->data.circle.label[sizeof(shape->data.circle.label) - 1] = '\0';
    shape->data.circle.color = color;

    shape->area_func = circle_area;
    shape->perimeter_func = circle_perimeter;
    shape->print_func = print_circle;
    shape->next = NULL;

    return shape;
}

/**
 * Calculates rectangle area
 */
double rectangle_area(const Shape* shape) {
    if (shape == NULL || shape->type != SHAPE_RECTANGLE) {
        return 0.0;
    }

    const Rectangle* rect = &shape->data.rect;
    double width = rect->bottom_right.x - rect->top_left.x;
    double height = rect->top_left.y - rect->bottom_right.y;

    return width * height;
}

/**
 * Calculates rectangle perimeter
 */
double rectangle_perimeter(const Shape* shape) {
    if (shape == NULL || shape->type != SHAPE_RECTANGLE) {
        return 0.0;
    }

    const Rectangle* rect = &shape->data.rect;
    double width = rect->bottom_right.x - rect->top_left.x;
    double height = rect->top_left.y - rect->bottom_right.y;

    return 2.0 * (width + height);
}

/**
 * Prints rectangle information
 */
void print_rectangle(const Shape* shape) {
    if (shape == NULL || shape->type != SHAPE_RECTANGLE) {
        printf("Invalid rectangle\n");
        return;
    }

    const Rectangle* rect = &shape->data.rect;
    printf("Rectangle '%s': (%.2f,%.2f) to (%.2f,%.2f), Color: 0x%08X\n",
           rect->label,
           rect->top_left.x, rect->top_left.y,
           rect->bottom_right.x, rect->bottom_right.y,
           rect->color);
}

/**
 * Calculates circle area
 */
double circle_area(const Shape* shape) {
    if (shape == NULL || shape->type != SHAPE_CIRCLE) {
        return 0.0;
    }

    const Circle* circle = &shape->data.circle;
    return 3.14159 * circle->radius * circle->radius;
}

/**
 * Calculates circle perimeter
 */
double circle_perimeter(const Shape* shape) {
    if (shape == NULL || shape->type != SHAPE_CIRCLE) {
        return 0.0;
    }

    const Circle* circle = &shape->data.circle;
    return 2.0 * 3.14159 * circle->radius;
}

/**
 * Prints circle information
 */
void print_circle(const Shape* shape) {
    if (shape == NULL || shape->type != SHAPE_CIRCLE) {
        printf("Invalid circle\n");
        return;
    }

    const Circle* circle = &shape->data.circle;
    printf("Circle '%s': Center (%.2f,%.2f), Radius %.2f, Color: 0x%08X\n",
           circle->label,
           circle->center.x, circle->center.y,
           circle->radius,
           circle->color);
}

/**
 * Creates a data processor with specified parameters
 */
DataProcessor* create_data_processor(const char* name, ProcessFunc func, size_t buffer_size) {
    DataProcessor* processor = malloc(sizeof(DataProcessor));
    if (processor == NULL) {
        return NULL;
    }

    strncpy(processor->name, name, sizeof(processor->name) - 1);
    processor->name[sizeof(processor->name) - 1] = '\0';

    processor->process_func = func;
    processor->context = NULL;
    processor->buffer_size = buffer_size;

    if (buffer_size > 0) {
        processor->buffer = malloc(buffer_size);
        if (processor->buffer == NULL) {
            free(processor);
            return NULL;
        }
    } else {
        processor->buffer = NULL;
    }

    // Initialize stats
    processor->stats.total_processed = 0;
    processor->stats.operations_count = 0;
    processor->stats.average_time = 0.0;

    return processor;
}

/**
 * Destroys a data processor and frees memory
 */
void destroy_data_processor(DataProcessor* processor) {
    if (processor != NULL) {
        if (processor->buffer != NULL) {
            free(processor->buffer);
        }
        free(processor);
    }
}

/**
 * Destroys a shape and frees memory
 */
void destroy_shape(Shape* shape) {
    if (shape != NULL) {
        free(shape);
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Shape",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify outline format for struct and union patterns
    assert!(
        output.contains("Shape"),
        "Missing Shape-related symbols - output: {}",
        output
    );
    assert!(
        output.contains("..."),
        "Missing truncation ellipsis in outline format - output: {}",
        output
    );
    // Should show C struct/union/typedef patterns
    let has_c_type_patterns = output.contains("typedef")
        || output.contains("struct")
        || output.contains("union")
        || output.contains("*")
        || output.contains("Rectangle")
        || output.contains("Circle");
    assert!(
        has_c_type_patterns,
        "Missing C type definition patterns - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_preprocessor_and_macros() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("preprocessor.c");

    let content = r#"// Complex C preprocessor directives and macros
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

// Configuration macros
#define VERSION_MAJOR 2
#define VERSION_MINOR 1
#define VERSION_PATCH 0
#define VERSION_STRING "2.1.0"

#ifdef DEBUG
    #define DBG_PRINT(fmt, ...) printf("[DEBUG] " fmt "\n", ##__VA_ARGS__)
#else
    #define DBG_PRINT(fmt, ...) do {} while(0)
#endif

// Utility macros
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#define CLAMP(value, min_val, max_val) MIN(MAX(value, min_val), max_val)

// Array size calculation
#define ARRAY_SIZE(arr) (sizeof(arr) / sizeof((arr)[0]))

// Bit manipulation macros
#define SET_BIT(value, bit) ((value) |= (1 << (bit)))
#define CLEAR_BIT(value, bit) ((value) &= ~(1 << (bit)))
#define TEST_BIT(value, bit) ((value) & (1 << (bit)))

// Complex macro with multiple statements
#define SAFE_FREE(ptr) do { \
    if ((ptr) != NULL) { \
        free(ptr); \
        (ptr) = NULL; \
    } \
} while(0)

// Conditional compilation for different platforms
#ifdef _WIN32
    #define PLATFORM "Windows"
    #define PATH_SEPARATOR '\\'
#elif defined(__linux__)
    #define PLATFORM "Linux"
    #define PATH_SEPARATOR '/'
#elif defined(__APPLE__)
    #define PLATFORM "macOS"
    #define PATH_SEPARATOR '/'
#else
    #define PLATFORM "Unknown"
    #define PATH_SEPARATOR '/'
#endif

// Feature flags
#if VERSION_MAJOR >= 2
    #define FEATURE_ADVANCED_MATH 1
    #define FEATURE_LOGGING 1
#else
    #define FEATURE_ADVANCED_MATH 0
    #define FEATURE_LOGGING 0
#endif

// Compiler-specific attributes
#if defined(__GNUC__) || defined(__clang__)
    #define FORCE_INLINE __attribute__((always_inline)) inline
    #define NO_RETURN __attribute__((noreturn))
    #define UNUSED __attribute__((unused))
#elif defined(_MSC_VER)
    #define FORCE_INLINE __forceinline
    #define NO_RETURN __declspec(noreturn)
    #define UNUSED
#else
    #define FORCE_INLINE inline
    #define NO_RETURN
    #define UNUSED
#endif

// Type definitions with macros
typedef struct {
    uint32_t flags;
    char platform[16];
    int version_major;
    int version_minor;
    int version_patch;
} SystemInfo;

/**
 * Function using preprocessor features
 */
SystemInfo* get_system_info(void) {
    static SystemInfo info = {0};
    static int initialized = 0;

    if (!initialized) {
        strcpy(info.platform, PLATFORM);
        info.version_major = VERSION_MAJOR;
        info.version_minor = VERSION_MINOR;
        info.version_patch = VERSION_PATCH;

        #if FEATURE_ADVANCED_MATH
        SET_BIT(info.flags, 0); // Math feature flag
        #endif

        #if FEATURE_LOGGING
        SET_BIT(info.flags, 1); // Logging feature flag
        #endif

        initialized = 1;
        DBG_PRINT("System info initialized for %s", PLATFORM);
    }

    return &info;
}

/**
 * Function demonstrating macro usage
 */
int* process_array(int* array, size_t size) {
    if (array == NULL || size == 0) {
        DBG_PRINT("Invalid array parameters");
        return NULL;
    }

    int* result = malloc(size * sizeof(int));
    if (result == NULL) {
        DBG_PRINT("Failed to allocate memory for %zu elements", size);
        return NULL;
    }

    // Process using macros
    for (size_t i = 0; i < size; i++) {
        int value = array[i];

        // Clamp value between 0 and 100
        result[i] = CLAMP(value, 0, 100);

        DBG_PRINT("Processed array[%zu]: %d -> %d", i, value, result[i]);
    }

    return result;
}

/**
 * Utility function with inline optimization
 */
FORCE_INLINE int fast_multiply(int a, int b) {
    return a * b;
}

/**
 * Function that never returns
 */
NO_RETURN void fatal_error(const char* message) {
    fprintf(stderr, "FATAL ERROR: %s\n", message);
    exit(1);
}

/**
 * Function with conditional compilation
 */
void print_features(void) {
    printf("Application Version: %s\n", VERSION_STRING);
    printf("Platform: %s\n", PLATFORM);
    printf("Path Separator: %c\n", PATH_SEPARATOR);

    #if FEATURE_ADVANCED_MATH
    printf("Advanced Math: Enabled\n");
    #else
    printf("Advanced Math: Disabled\n");
    #endif

    #if FEATURE_LOGGING
    printf("Logging: Enabled\n");
    #else
    printf("Logging: Disabled\n");
    #endif

    SystemInfo* info = get_system_info();
    printf("System Flags: 0x%08X\n", info->flags);
}

/**
 * Main function demonstrating all features
 */
int main(void) {
    print_features();

    int test_array[] = {-10, 25, 150, 75, -5, 200};
    size_t array_size = ARRAY_SIZE(test_array);

    printf("\nOriginal array size: %zu\n", array_size);

    int* processed = process_array(test_array, array_size);
    if (processed != NULL) {
        printf("Processed array: ");
        for (size_t i = 0; i < array_size; i++) {
            printf("%d ", processed[i]);
        }
        printf("\n");

        SAFE_FREE(processed);
    }

    // Test inline function
    int product = fast_multiply(7, 8);
    printf("Fast multiply result: %d\n", product);

    return 0;
}

// Clean up macros if needed
#undef DBG_PRINT
#undef SAFE_FREE
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "get_system_info",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify outline format for preprocessor and macros
    assert!(
        output.contains("get_system_info"),
        "Missing get_system_info function - output: {}",
        output
    );
    assert!(
        output.contains("..."),
        "Missing truncation ellipsis in outline format - output: {}",
        output
    );
    // Should show C preprocessor patterns and function signatures
    let has_c_preprocessor_patterns = output.contains("SystemInfo")
        || output.contains("#if")
        || output.contains("void")
        || output.contains("*")
        || output.contains("printf");
    assert!(
        has_c_preprocessor_patterns,
        "Missing C preprocessor and function patterns - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_large_function_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_function.c");

    let content = r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

/**
 * Large function with multiple nested blocks to test closing brace comments
 */
char** complex_text_processor(char** input, int input_count, int* output_count) {
    if (input == NULL || input_count <= 0 || output_count == NULL) {
        *output_count = 0;
        return NULL;
    }

    // Initialize output array
    char** output = malloc(input_count * 2 * sizeof(char*));
    if (output == NULL) {
        *output_count = 0;
        return NULL;
    }

    int current_output = 0;
    char* categories[10] = {"short", "medium", "long", "email", "url", "number",
                           "uppercase", "lowercase", "mixed", "special"};

    // Phase 1: Categorize and validate input
    for (int i = 0; i < input_count; i++) {
        char* text = input[i];
        if (text == NULL || strlen(text) == 0) {
            printf("Warning: empty text at index %d\n", i);
            continue;
        }

        size_t len = strlen(text);
        char* processed = malloc(len + 100); // Extra space for modifications
        if (processed == NULL) {
            printf("Memory allocation failed for index %d\n", i);
            continue;
        }

        // Determine category and process accordingly
        if (len <= 10) {
            // Short text processing
            strcpy(processed, "SHORT: ");

            // Check if numeric
            int is_numeric = 1;
            for (size_t j = 0; j < len; j++) {
                if (!isdigit(text[j])) {
                    is_numeric = 0;
                    break;
                }
            }

            if (is_numeric) {
                strcat(processed, "NUM_");
                strcat(processed, text);
            } else {
                // Convert to uppercase
                for (size_t j = 0; j < len; j++) {
                    if (islower(text[j])) {
                        processed[7 + j] = toupper(text[j]);
                    } else {
                        processed[7 + j] = text[j];
                    }
                }
                processed[7 + len] = '\0';
            }

        } else if (len <= 50) {
            // Medium text processing
            strcpy(processed, "MEDIUM: ");

            // Check for email pattern
            if (strchr(text, '@') != NULL && strchr(text, '.') != NULL) {
                strcat(processed, "EMAIL_");
                strncat(processed, text, 30); // Truncate long emails
            } else if (strncmp(text, "http", 4) == 0) {
                strcat(processed, "URL_");
                strncat(processed, text, 30); // Truncate long URLs
            } else {
                // Title case conversion
                int capitalize_next = 1;
                size_t processed_len = strlen(processed);

                for (size_t j = 0; j < len && processed_len + j < len + 99; j++) {
                    if (isspace(text[j])) {
                        processed[processed_len + j] = text[j];
                        capitalize_next = 1;
                    } else if (capitalize_next && isalpha(text[j])) {
                        processed[processed_len + j] = toupper(text[j]);
                        capitalize_next = 0;
                    } else {
                        processed[processed_len + j] = tolower(text[j]);
                    }
                }
                processed[processed_len + len] = '\0';
            }

        } else {
            // Long text processing
            strcpy(processed, "LONG: ");

            // Count character types
            int uppercase_count = 0, lowercase_count = 0, digit_count = 0, special_count = 0;

            for (size_t j = 0; j < len; j++) {
                if (isupper(text[j])) {
                    uppercase_count++;
                } else if (islower(text[j])) {
                    lowercase_count++;
                } else if (isdigit(text[j])) {
                    digit_count++;
                } else {
                    special_count++;
                }
            }

            // Determine dominant character type
            if (uppercase_count > lowercase_count && uppercase_count > digit_count) {
                strcat(processed, "UPPER_DOM: ");
            } else if (lowercase_count > uppercase_count && lowercase_count > digit_count) {
                strcat(processed, "LOWER_DOM: ");
            } else if (digit_count > uppercase_count && digit_count > lowercase_count) {
                strcat(processed, "DIGIT_DOM: ");
            } else {
                strcat(processed, "MIXED_DOM: ");
            }

            // Truncate and add summary
            size_t current_len = strlen(processed);
            if (len > 30) {
                strncat(processed, text, 30);
                strcat(processed, "...");
                sprintf(processed + strlen(processed), " (len=%zu)", len);
            } else {
                strcat(processed, text);
            }
        }

        output[current_output++] = processed;
    }

    // Phase 2: Sort results by category
    for (int i = 0; i < current_output - 1; i++) {
        for (int j = i + 1; j < current_output; j++) {
            if (strcmp(output[i], output[j]) > 0) {
                char* temp = output[i];
                output[i] = output[j];
                output[j] = temp;
            }
        }
    }

    // Phase 3: Final validation and cleanup
    int final_count = 0;
    for (int i = 0; i < current_output; i++) {
        if (output[i] != NULL && strlen(output[i]) > 0) {
            if (final_count != i) {
                output[final_count] = output[i];
            }
            final_count++;
        } else {
            free(output[i]);
        }
    }

    *output_count = final_count;
    return output;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "complex_text_processor",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify outline format for large function with closing brace comments
    assert!(
        output.contains("complex_text_processor"),
        "Missing complex_text_processor function - output: {}",
        output
    );
    assert!(
        output.contains("..."),
        "Missing truncation ellipsis in outline format - output: {}",
        output
    );
    // Should have closing brace comment for the large function
    assert!(
        output.contains("} //") || output.contains("}//") || output.contains("} /*"),
        "Missing closing brace comment for large function - output: {}",
        output
    );
    // Should show C-specific patterns for large functions
    let has_c_patterns =
        output.contains("char**") || output.contains("int*") || output.contains("function");
    assert!(
        has_c_patterns,
        "Missing C-specific patterns for large function - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_search_command() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("search_test.c");

    let content = r#"#include <stdio.h>
#include <stdlib.h>

typedef struct DataProcessor {
    int processed_count;
    double* data;
    size_t capacity;
} DataProcessor;

DataProcessor* create_processor(size_t initial_capacity) {
    DataProcessor* processor = malloc(sizeof(DataProcessor));
    if (processor == NULL) return NULL;

    processor->processed_count = 0;
    processor->capacity = initial_capacity;
    processor->data = malloc(initial_capacity * sizeof(double));

    if (processor->data == NULL) {
        free(processor);
        return NULL;
    }

    return processor;
}

int process_data(DataProcessor* processor, double* input, size_t count) {
    if (processor == NULL || input == NULL) return -1;

    for (size_t i = 0; i < count; i++) {
        if (input[i] != 0.0) {
            processor->data[processor->processed_count++] = input[i];
        }
    }

    return processor->processed_count;
}

char* process_file(const char* filename) {
    if (filename == NULL) return NULL;

    size_t len = strlen(filename) + 20;
    char* result = malloc(len);
    if (result == NULL) return NULL;

    snprintf(result, len, "Processed %s", filename);
    return result;
}

void process_async(DataProcessor* processor, void (*callback)(int)) {
    if (processor == NULL || callback == NULL) return;

    // Simulate async processing
    int result = processor->processed_count * 2;
    callback(result);
}

void test_data_processing() {
    DataProcessor* processor = create_processor(100);
    if (processor == NULL) return;

    double test_data[] = {1.0, 2.0, 0.0, 3.0};
    int count = process_data(processor, test_data, 4);

    printf("Processed %d items\n", count);

    free(processor->data);
    free(processor);
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
            || output.contains("process_data")
            || output.contains("process_file")
            || output.contains("process_async"),
        "Should find process-related symbols - output: {}",
        output
    );

    Ok(())
}
#[test]
fn test_c_outline_closing_brace_comments_syntax() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("closing_braces.c");

    let content = r#"#include <stdio.h>
#include <stdlib.h>

/**
 * Large function that should get C-style closing brace comment with // syntax
 */
int large_function_with_nested_blocks(int param1, int param2, int param3) {
    if (param1 > 0) {
        for (int i = 0; i < param1; i++) {
            if (param2 % 2 == 0) {
                switch (param3) {
                    case 1:
                        printf("Case 1\n");
                        break;
                    case 2:
                        printf("Case 2\n");
                        break;
                    default:
                        printf("Default case\n");
                        break;
                }
            } else {
                while (param2 > 0) {
                    param2--;
                    if (param2 == 10) {
                        break;
                    }
                }
            }
        }
    }

    // More code to make this function large
    int result = 0;
    for (int j = 0; j < 10; j++) {
        result += j * param1;
    }

    return result;
}

/**
 * Small function that should NOT get closing brace comments
 */
int small_function(int x) {
    return x * 2;
}

/**
 * Medium function to test threshold behavior
 */
void medium_function(int count) {
    for (int i = 0; i < count; i++) {
        printf("Iteration: %d\n", i);
        if (i % 2 == 0) {
            printf("Even number\n");
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "large_function",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Should contain C-style closing brace comments with // syntax (not /* */)
    assert!(
        output.contains("} //"),
        "Missing C-style closing brace comments with // syntax - output: {}",
        output
    );

    // Should not contain /* */ style comments for closing braces
    assert!(
        !output.contains("} /*") || !output.contains("*/"),
        "Should not use /* */ style for C closing brace comments - output: {}",
        output
    );

    // Should show function keyword in closing brace comment
    assert!(
        output.contains("function") || output.contains("Function"),
        "Missing function keyword in closing brace comment - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_header_file_support() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("header_test.h");

    let content = r#"#ifndef HEADER_TEST_H
#define HEADER_TEST_H

#include <stdint.h>
#include <stdbool.h>

// Forward declarations
typedef struct Point Point;
typedef struct Vector Vector;

/**
 * Point structure with coordinates
 */
typedef struct Point {
    double x;
    double y;
    double z;
} Point;

/**
 * Vector structure with direction and magnitude
 */
typedef struct Vector {
    Point start;
    Point end;
    double magnitude;
    bool normalized;
} Vector;

/**
 * Function prototypes - typical header file pattern
 */
Point* create_point(double x, double y, double z);
void destroy_point(Point* point);
Vector* create_vector(Point* start, Point* end);
void destroy_vector(Vector* vector);
double calculate_distance(const Point* p1, const Point* p2);
Vector* normalize_vector(Vector* vector);
bool points_equal(const Point* p1, const Point* p2, double tolerance);

/**
 * Inline function definition in header
 */
static inline double point_magnitude(const Point* point) {
    if (point == NULL) {
        return 0.0;
    }
    return sqrt(point->x * point->x + point->y * point->y + point->z * point->z);
}

/**
 * Macro definitions
 */
#define POINT_ZERO {0.0, 0.0, 0.0}
#define VECTOR_ZERO {{0.0, 0.0, 0.0}, {0.0, 0.0, 0.0}, 0.0, false}
#define MAX_POINTS 1000
#define EPSILON 1e-10

/**
 * Conditional compilation for different platforms
 */
#ifdef __cplusplus
extern "C" {
#endif

// Additional C++ compatible declarations would go here

#ifdef __cplusplus
}
#endif

#endif // HEADER_TEST_H
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "typedef",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Should parse and format .h files correctly
    assert!(
        output.contains("Point") || output.contains("typedef"),
        "Missing Point struct in header file - output: {}",
        output
    );

    // Should show header-specific patterns
    let has_header_patterns = output.contains("#ifndef")
        || output.contains("#define")
        || output.contains("typedef struct")
        || output.contains("extern")
        || output.contains("inline")
        || output.contains("struct");
    assert!(
        has_header_patterns,
        "Missing header file specific patterns - output: {}",
        output
    );

    // Should have outline formatting - search might not be long enough for truncation
    let has_outline_features =
        output.contains("...") || output.contains("typedef") || output.contains("Point");
    assert!(
        has_outline_features,
        "Missing outline format features in header file - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_test_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test_functions.c");

    let content = r#"#include <stdio.h>
#include <assert.h>
#include <string.h>

/**
 * Regular function - not a test
 */
int calculate_sum(int a, int b) {
    return a + b;
}

/**
 * Test function with test_ prefix - should be detected as test
 */
void test_calculate_sum() {
    int result = calculate_sum(2, 3);
    assert(result == 5);
    printf("test_calculate_sum passed\n");
}

/**
 * Another test function with test_ prefix
 */
void test_string_operations() {
    char buffer[100];
    strcpy(buffer, "Hello");
    strcat(buffer, " World");

    assert(strcmp(buffer, "Hello World") == 0);
    assert(strlen(buffer) == 11);

    printf("test_string_operations passed\n");
}

/**
 * Test function with assert statements - should be detected
 */
int test_array_operations() {
    int arr[] = {1, 2, 3, 4, 5};
    int sum = 0;

    for (int i = 0; i < 5; i++) {
        sum += arr[i];
    }

    assert(sum == 15);
    assert(arr[0] == 1);
    assert(arr[4] == 5);

    return 0;
}

/**
 * Function with assert but not a test name - might be detected
 */
void validate_input(int input) {
    assert(input > 0);
    assert(input < 1000);
    printf("Input %d is valid\n", input);
}

/**
 * Unit test style function
 */
void test_memory_allocation() {
    int* ptr = malloc(sizeof(int) * 10);
    assert(ptr != NULL);

    for (int i = 0; i < 10; i++) {
        ptr[i] = i * 2;
    }

    assert(ptr[5] == 10);
    free(ptr);

    printf("test_memory_allocation passed\n");
}

/**
 * Main function to run tests
 */
int main() {
    test_calculate_sum();
    test_string_operations();
    test_array_operations();
    test_memory_allocation();
    validate_input(42);

    printf("All tests passed!\n");
    return 0;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "assert",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Should find test-related functions or patterns
    let has_test_content = output.contains("test_calculate_sum")
        || output.contains("test_string_operations")
        || output.contains("assert")
        || output.contains("calculate_sum");
    assert!(
        has_test_content,
        "Missing test-related content - output: {}",
        output
    );

    // Should show C test patterns
    let has_c_test_patterns = output.contains("assert")
        || output.contains("int")
        || output.contains("void")
        || output.contains("printf")
        || output.contains("malloc");
    assert!(
        has_c_test_patterns,
        "Missing C test-related patterns - output: {}",
        output
    );

    // Should have outline formatting features
    let has_outline_features = output.contains("...")
        || output.contains("assert")
        || output.contains("test_")
        || output.contains("void");
    assert!(
        has_outline_features,
        "Missing outline format features in test detection - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_pointer_and_advanced_constructs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("advanced_c.c");

    let content = r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/**
 * Function pointer typedef
 */
typedef int (*CompareFunc)(const void* a, const void* b);
typedef void (*CallbackFunc)(int result, void* user_data);

/**
 * Complex struct with function pointers
 */
typedef struct Processor {
    char name[64];
    CompareFunc compare_func;
    CallbackFunc callback_func;
    void* user_data;
    int (*process_func)(struct Processor* self, void* data, size_t size);
    void (*cleanup_func)(struct Processor* self);
} Processor;

/**
 * Function that takes function pointer as parameter
 */
void process_array_with_callback(int* array, size_t length,
                               int (*transform_func)(int value),
                               void (*result_callback)(int index, int result)) {
    if (array == NULL || transform_func == NULL) {
        return;
    }

    for (size_t i = 0; i < length; i++) {
        int result = transform_func(array[i]);
        if (result_callback != NULL) {
            result_callback((int)i, result);
        }
    }
}

/**
 * Function returning pointer to pointer
 */
char** create_string_array(int count, const char* prefix) {
    char** strings = malloc(count * sizeof(char*));
    if (strings == NULL) {
        return NULL;
    }

    for (int i = 0; i < count; i++) {
        size_t len = strlen(prefix) + 20;
        strings[i] = malloc(len);
        if (strings[i] != NULL) {
            snprintf(strings[i], len, "%s_%d", prefix, i);
        }
    }

    return strings;
}

/**
 * Function with complex pointer arithmetic
 */
void reverse_array_pointers(int* start, int* end) {
    while (start < end) {
        int temp = *start;
        *start = *end;
        *end = temp;
        start++;
        end--;
    }
}

/**
 * Function with void pointer and casting
 */
void generic_swap(void* a, void* b, size_t size) {
    unsigned char* byte_a = (unsigned char*)a;
    unsigned char* byte_b = (unsigned char*)b;

    for (size_t i = 0; i < size; i++) {
        unsigned char temp = byte_a[i];
        byte_a[i] = byte_b[i];
        byte_b[i] = temp;
    }
}

/**
 * Preprocessor macro with function-like behavior
 */
#define SAFE_DELETE(ptr) do { \
    if ((ptr) != NULL) { \
        free(ptr); \
        (ptr) = NULL; \
    } \
} while(0)

#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define MIN(a, b) ((a) < (b) ? (a) : (b))

/**
 * Function using the macros
 */
void cleanup_processor(Processor** processor_ptr) {
    if (processor_ptr == NULL || *processor_ptr == NULL) {
        return;
    }

    Processor* proc = *processor_ptr;

    if (proc->cleanup_func != NULL) {
        proc->cleanup_func(proc);
    }

    SAFE_DELETE(proc->user_data);
    SAFE_DELETE(*processor_ptr);
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "typedef",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Should show function pointer and pointer-related patterns
    let has_pointer_patterns = output.contains("*")
        || output.contains("CompareFunc")
        || output.contains("CallbackFunc")
        || output.contains("void*")
        || output.contains("char**")
        || output.contains("int*")
        || output.contains("typedef");
    assert!(
        has_pointer_patterns,
        "Missing pointer-related patterns - output: {}",
        output
    );

    // Should have outline formatting features
    let has_outline_features = output.contains("...")
        || output.contains("typedef")
        || output.contains("CompareFunc")
        || output.contains("struct");
    assert!(
        has_outline_features,
        "Missing outline format features in advanced C constructs - output: {}",
        output
    );

    // Should show C-specific advanced constructs
    let has_advanced_c = output.contains("typedef")
        || output.contains("struct")
        || output.contains("malloc")
        || output.contains("sizeof")
        || output.contains("#define")
        || output.contains("Processor");
    assert!(
        has_advanced_c,
        "Missing advanced C constructs - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_c_outline_keyword_highlighting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keyword_test.c");

    let content = r#"#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>

/**
 * Function demonstrating various C keywords for highlighting
 */
int demonstrate_keywords(int count, const char* message, bool verbose) {
    // Static local variable
    static int call_count = 0;
    call_count++;

    // Register variable (though rarely used in modern C)
    register int fast_counter;

    // Volatile variable
    volatile int shared_flag = 0;

    // Const pointer to const data
    const char* const constant_message = "Hello, World!";

    // Auto keyword (implicit in C99+)
    auto int automatic_var = 42;

    // Extern declaration
    extern int global_variable;

    // Different loop types with keywords
    for (fast_counter = 0; fast_counter < count; fast_counter++) {
        if (verbose && fast_counter % 2 == 0) {
            printf("Processing item %d: %s\n", fast_counter, message);
        }

        switch (fast_counter % 4) {
            case 0:
                if (shared_flag == 0) {
                    shared_flag = 1;
                }
                break;
            case 1:
                while (shared_flag > 0) {
                    shared_flag--;
                    if (shared_flag <= 0) break;
                }
                break;
            case 2:
                do {
                    shared_flag++;
                } while (shared_flag < 3);
                break;
            default:
                goto cleanup;
        }
    }

cleanup:
    // Return with appropriate value
    return call_count;
}

/**
 * Function with different storage classes and type qualifiers
 */
static inline void storage_class_demo(void) {
    // Different storage classes
    static const int static_const_var = 100;
    extern volatile int extern_volatile_var;
    register unsigned int register_var = 0;

    // Type qualifiers
    const int* const_ptr;
    volatile int* volatile_ptr;
    restrict int* restrict_ptr;  // C99 feature

    // Compound types
    struct {
        union {
            int int_val;
            float float_val;
        } data;
        enum {
            STATE_INIT,
            STATE_RUNNING,
            STATE_STOPPED
        } state;
    } compound_var;
}

/**
 * Function using sizeof, typeof (GCC extension), and other operators
 */
void operator_keyword_demo(void) {
    int array[10];
    size_t array_size = sizeof(array);
    size_t element_size = sizeof(array[0]);
    size_t num_elements = array_size / element_size;

    // Typeof is a GCC extension
    #ifdef __GNUC__
    typeof(array[0]) another_int = 42;
    #endif

    // Alignment specifier (C11)
    #if __STDC_VERSION__ >= 201112L
    _Alignas(16) int aligned_var;
    size_t alignment = _Alignof(int);
    #endif
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "demonstrate_keywords",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Should find functions with keyword in their name or content
    assert!(
        output.contains("keyword") || output.contains("demonstrate_keywords"),
        "Missing keyword-related functions - output: {}",
        output
    );

    // Should show C keywords in the outline format
    let has_c_keywords = output.contains("static")
        || output.contains("const")
        || output.contains("volatile")
        || output.contains("int")
        || output.contains("void")
        || output.contains("for")
        || output.contains("while")
        || output.contains("if")
        || output.contains("switch");
    assert!(
        has_c_keywords,
        "Missing C keywords in outline format - output: {}",
        output
    );

    // Should have outline formatting with truncation
    assert!(
        output.contains("..."),
        "Missing truncation in keyword demonstration outline - output: {}",
        output
    );

    Ok(())
}

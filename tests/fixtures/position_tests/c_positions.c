// Test fixture for C tree-sitter position validation
// Line numbers and symbol positions are tested precisely

#include <stdio.h>
#include <stdlib.h>

void simple_function() {} // simple_function at position (line 7, col 5)

int function_with_return() { // function_with_return at position (line 9, col 4)
    return 42;
}

void function_with_params(int param1, char *param2) { // function_with_params at position (line 13, col 5)
    printf("%d %s\n", param1, param2);
}

static void static_function() { // static_function at position (line 17, col 12)
    printf("Static function\n");
}

extern void extern_function(); // extern_function at position (line 21, col 12)

inline int inline_function(int x) { // inline_function at position (line 23, col 11)
    return x * 2;
}

struct SimpleStruct { // SimpleStruct at position (line 27, col 7)
    int field1;   // field1 at position (line 28, col 8)
    char *field2; // field2 at position (line 29, col 10)
};

typedef struct { // This creates an anonymous struct
    int x;       // x at position (line 33, col 8)
    int y;       // y at position (line 34, col 8)
} Point; // Point at position (line 35, col 2)

typedef struct NamedStruct { // NamedStruct at position (line 37, col 15)
    float value;             // value at position (line 38, col 10)
} NamedStructAlias; // NamedStructAlias at position (line 39, col 2)

union SimpleUnion { // SimpleUnion at position (line 41, col 6)
    int i;      // i at position (line 42, col 8)
    float f;    // f at position (line 43, col 10)
    char c[4];  // c at position (line 44, col 9)
};

typedef union { // Anonymous union
    int integer;    // integer at position (line 48, col 8)
    float decimal;  // decimal at position (line 49, col 10)
} Number; // Number at position (line 50, col 2)

enum Color { // Color at position (line 52, col 5)
    RED,     // RED at position (line 53, col 4)
    GREEN,   // GREEN at position (line 54, col 4)
    BLUE     // BLUE at position (line 55, col 4)
};

typedef enum { // Anonymous enum
    SMALL,     // SMALL at position (line 59, col 4)
    MEDIUM,    // MEDIUM at position (line 60, col 4)
    LARGE      // LARGE at position (line 61, col 4)
} Size; // Size at position (line 62, col 2)

// Function pointer type
typedef int (*FunctionPtr)(int, int); // FunctionPtr at position (line 65, col 13)

// Global variables
int global_var = 42;        // global_var at position (line 68, col 4)
static int static_var = 0;  // static_var at position (line 69, col 11)
extern int extern_var;      // extern_var at position (line 70, col 11)
const int const_var = 100;  // const_var at position (line 71, col 10)

// Preprocessor definitions
#define MAX_SIZE 1024       // MAX_SIZE at position (line 74, col 8)
#define SQUARE(x) ((x) * (x)) // SQUARE at position (line 75, col 8)

// Function-like macros are handled differently by tree-sitter
#define DEBUG_PRINT(fmt, ...) \
    printf("DEBUG: " fmt "\n", ##__VA_ARGS__)

int main() { // main at position (line 80, col 4)
    return 0;
}

// Function with array parameters
void array_function(int arr[], int size) { // array_function at position (line 84, col 5)
    // implementation
}

// Function with pointer parameters
void pointer_function(int *ptr, char **argv) { // pointer_function at position (line 88, col 5)
    // implementation
}

// Variadic function
void variadic_function(int count, ...) { // variadic_function at position (line 92, col 5)
    // implementation
}
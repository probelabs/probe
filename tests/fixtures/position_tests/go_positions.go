// Test fixture for Go tree-sitter position validation
// Line numbers and symbol positions are tested precisely

package main

import "fmt"

func simpleFunction() {} // simpleFunction at position (line 8, col 5)

func functionWithParams(param1 int, param2 string) string {
    return fmt.Sprintf("%d %s", param1, param2)
} // functionWithParams at position (line 10, col 5)

func functionWithReturn() int {
    return 42
} // functionWithReturn at position (line 14, col 5)

func multipleReturns() (int, error) {
    return 42, nil
} // multipleReturns at position (line 18, col 5)

type SimpleStruct struct {
    Field1 int
    Field2 string
} // SimpleStruct at position (line 22, col 5)

type InterfaceType interface {
    Method1() int           // Method1 at position (line 27, col 4)
    Method2(string) error   // Method2 at position (line 28, col 4)
} // InterfaceType at position (line 26, col 5)

func (s SimpleStruct) Method() int {
    return s.Field1
} // Method at position (line 31, col 22)

func (s *SimpleStruct) PointerMethod() {
    s.Field1 += 1
} // PointerMethod at position (line 35, col 23)

type CustomInt int

func (c CustomInt) String() string {
    return fmt.Sprintf("CustomInt(%d)", c)
} // String at position (line 41, col 19)

const CONSTANT = 42 // CONSTANT at position (line 45, col 6)

const (
    CONST1 = 1  // CONST1 at position (line 48, col 4)
    CONST2 = 2  // CONST2 at position (line 49, col 4)
    CONST3 = 3  // CONST3 at position (line 50, col 4)
)

var globalVar = "hello" // globalVar at position (line 53, col 4)

var (
    var1 = 1    // var1 at position (line 56, col 4)
    var2 = 2    // var2 at position (line 57, col 4)
    var3 = 3    // var3 at position (line 58, col 4)
)

type (
    TypeAlias1 = string     // TypeAlias1 at position (line 62, col 4)
    TypeAlias2 = int        // TypeAlias2 at position (line 63, col 4)
)

func init() {
    fmt.Println("Init function")
} // init at position (line 66, col 5)

func main() {
    fmt.Println("Main function")
} // main at position (line 70, col 5)

// Generic function (Go 1.18+)
func GenericFunction[T any](value T) T {
    return value
} // GenericFunction at position (line 75, col 5)

// Generic struct (Go 1.18+)
type GenericStruct[T any] struct {
    Value T
} // GenericStruct at position (line 80, col 5)

func (g GenericStruct[T]) GetValue() T {
    return g.Value
} // GetValue at position (line 84, col 30)
<?php
// Test fixture for PHP tree-sitter position validation
// Line numbers and symbol positions are tested precisely

namespace TestProject;

/**
 * Simple function for position testing
 */
function simpleFunction() {} // simpleFunction at position (line 9, col 9)

/**
 * Function with parameters for testing parameter position detection
 */
function functionWithParams(int $param1, string $param2): string {
    return sprintf("%d %s", $param1, $param2);
} // functionWithParams at position (line 14, col 9)

/**
 * Function with return type for testing return type position
 */
function functionWithReturn(): int {
    return 42;
} // functionWithReturn at position (line 20, col 9)

/**
 * Function with multiple return types
 */
function multipleReturns(): array {
    return [42, null];
} // multipleReturns at position (line 26, col 9)

/**
 * Simple class for testing class position detection
 */
class SimpleClass {
    public int $field1;
    public string $field2;

    /**
     * Public method for testing method position
     */
    public function publicMethod(): int {
        return $this->field1;
    } // publicMethod at position (line 40, col 19)

    /**
     * Private method for testing private method position
     */
    private function privateMethod(): void {
        $this->field1 += 1;
    } // privateMethod at position (line 46, col 20)

    /**
     * Static method for testing static method position
     */
    public static function staticMethod(): string {
        return "static";
    } // staticMethod at position (line 52, col 23)
} // SimpleClass at position (line 32, col 6)

/**
 * Interface for testing interface position detection
 */
interface SimpleInterface {
    /**
     * Interface method 1
     */
    public function method1(): int;          // method1 at position (line 62, col 19)

    /**
     * Interface method 2
     */
    public function method2(string $param): bool;   // method2 at position (line 67, col 19)
} // SimpleInterface at position (line 57, col 10)

/**
 * Abstract class for testing abstract class position
 */
abstract class AbstractClass {
    protected int $value;

    /**
     * Abstract method for testing abstract method position
     */
    abstract public function abstractMethod(): void; // abstractMethod at position (line 79, col 28)

    /**
     * Concrete method in abstract class
     */
    public function concreteMethod(): int {
        return $this->value;
    } // concreteMethod at position (line 85, col 19)
} // AbstractClass at position (line 72, col 15)

/**
 * Trait for testing trait position detection
 */
trait SimpleTrait {
    /**
     * Trait method for testing trait method position
     */
    public function traitMethod(): string {
        return "trait";
    } // traitMethod at position (line 97, col 19)
} // SimpleTrait at position (line 92, col 6)

/**
 * Class using trait for testing trait usage
 */
class ClassWithTrait {
    use SimpleTrait;

    /**
     * Regular method in class with trait
     */
    public function regularMethod(): void {
        echo $this->traitMethod();
    } // regularMethod at position (line 110, col 19)
} // ClassWithTrait at position (line 104, col 6)

// Constants for testing constant position detection
const GLOBAL_CONSTANT = 42; // GLOBAL_CONSTANT at position (line 116, col 6)

define('DEFINED_CONSTANT', 'value'); // DEFINED_CONSTANT at position (line 118, col 7)

// Global variables for testing variable position detection
$globalVar = "hello"; // globalVar at position (line 121, col 0)

/**
 * Class with constructor for testing constructor position
 */
class ClassWithConstructor {
    private int $value;

    /**
     * Constructor for testing constructor position
     */
    public function __construct(int $value) {
        $this->value = $value;
    } // __construct at position (line 131, col 19)

    /**
     * Destructor for testing destructor position
     */
    public function __destruct() {
        // cleanup
    } // __destruct at position (line 138, col 19)
} // ClassWithConstructor at position (line 126, col 6)

/**
 * Class with magic methods for testing magic method positions
 */
class ClassWithMagicMethods {
    private array $data = [];

    /**
     * Magic get method
     */
    public function __get(string $name) {
        return $this->data[$name] ?? null;
    } // __get at position (line 151, col 19)

    /**
     * Magic set method
     */
    public function __set(string $name, $value): void {
        $this->data[$name] = $value;
    } // __set at position (line 158, col 19)

    /**
     * Magic toString method
     */
    public function __toString(): string {
        return json_encode($this->data);
    } // __toString at position (line 165, col 19)
} // ClassWithMagicMethods at position (line 145, col 6)

/**
 * Namespace function for testing namespace function position
 */
function namespaceFunction(): void {
    echo "namespace function";
} // namespaceFunction at position (line 172, col 9)

/**
 * Anonymous class for testing anonymous class detection
 */
$anonymousClass = new class {
    public function anonymousMethod(): string {
        return "anonymous";
    } // anonymousMethod at position (line 180, col 19)
}; // anonymous class at position (line 178, col 18)

/**
 * Closure for testing closure position detection
 */
$closure = function(int $x): int {
    return $x * 2;
}; // closure at position (line 186, col 11)

/**
 * Arrow function for testing arrow function position (PHP 7.4+)
 */
$arrowFunction = fn(int $x): int => $x * 3; // arrowFunction at position (line 192, col 17)

/**
 * Enum for testing enum position detection (PHP 8.1+)
 */
enum SimpleEnum {
    case CASE1; // CASE1 at position (line 197, col 9)
    case CASE2; // CASE2 at position (line 198, col 9)
    case CASE3; // CASE3 at position (line 199, col 9)
} // SimpleEnum at position (line 195, col 5)

/**
 * Class with PHP 8 features for testing modern PHP positions
 */
class ModernPHPClass {
    /**
     * Constructor with property promotion
     */
    public function __construct(
        public readonly string $name,    // name at position (line 209, col 31)
        private int $value              // value at position (line 210, col 20)
    ) {} // __construct at position (line 207, col 19)

    /**
     * Method with union types
     */
    public function methodWithUnionTypes(string|int $param): string|null {
        return is_string($param) ? $param : (string)$param;
    } // methodWithUnionTypes at position (line 217, col 19)
} // ModernPHPClass at position (line 204, col 6)
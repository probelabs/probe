// Test fixture for TypeScript tree-sitter position validation
// Line numbers and symbol positions are tested precisely

function regularFunction(): void {} // regularFunction at position (line 3, col 9)

function functionWithTypes(param1: number, param2: string): string {
    return param1.toString() + param2;
} // functionWithTypes at position (line 5, col 9)

const arrowFunction = (): void => {}; // arrowFunction at position (line 9, col 6)

const arrowWithTypes = (x: number, y: number): number => x + y; // arrowWithTypes at position (line 11, col 6)

interface MyInterface {
    property: number;
    method(): string;       // method at position (line 15, col 4)
    optionalProp?: boolean; // optionalProp at position (line 16, col 4)
} // MyInterface at position (line 13, col 10)

type MyType = {
    x: number;
    y: string;
}; // MyType at position (line 19, col 5)

type UnionType = string | number; // UnionType at position (line 24, col 5)

class MyClass implements MyInterface {
    property: number;
    
    constructor(value: number) {
        this.property = value;
    } // constructor at position (line 30, col 4)
    
    method(): string {
        return this.property.toString();
    } // method at position (line 34, col 4)
    
    static staticMethod(): string {
        return "static";
    } // staticMethod at position (line 38, col 11)
    
    private privateMethod(): void {
        // private implementation
    } // privateMethod at position (line 42, col 12)
    
    public publicMethod(): void {
        // public implementation
    } // publicMethod at position (line 46, col 11)
    
    protected protectedMethod(): void {
        // protected implementation
    } // protectedMethod at position (line 50, col 14)
}

abstract class AbstractClass {
    abstract abstractMethod(): void; // abstractMethod at position (line 55, col 13)
    
    concreteMethod(): void {
        // concrete implementation
    } // concreteMethod at position (line 58, col 4)
} // AbstractClass at position (line 54, col 15)

enum Color {
    Red,    // Red at position (line 64, col 4)
    Green,  // Green at position (line 65, col 4)
    Blue    // Blue at position (line 66, col 4)
} // Color at position (line 63, col 5)

enum NumberEnum {
    First = 1,  // First at position (line 70, col 4)
    Second = 2, // Second at position (line 71, col 4)
    Third = 3   // Third at position (line 72, col 4)
} // NumberEnum at position (line 69, col 5)

namespace MyNamespace {
    export function namespacedFunction(): void {} // namespacedFunction at position (line 76, col 20)
    
    export const namespacedConst = 42; // namespacedConst at position (line 78, col 17)
} // MyNamespace at position (line 75, col 10)

const genericFunction = <T>(value: T): T => value; // genericFunction at position (line 81, col 6)

function overloadedFunction(param: string): string;
function overloadedFunction(param: number): number;
function overloadedFunction(param: any): any {
    return param;
} // overloadedFunction at position (line 85, col 9)

// Decorators (if enabled)
// @decorator
// class DecoratedClass {} // DecoratedClass would be at position

const varConstant: number = 42; // varConstant at position (line 91, col 6)

let varVariable: string = "hello"; // varVariable at position (line 93, col 4)

// Export statements
export function exportedFunction(): void {} // exportedFunction at position (line 96, col 16)

export const exportedConst: number = 42; // exportedConst at position (line 98, col 13)

export default function defaultExport(): void {} // defaultExport at position (line 100, col 24)
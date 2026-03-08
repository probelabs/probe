// Test fixture for JavaScript tree-sitter position validation
// Line numbers and symbol positions are tested precisely

function regularFunction() {} // regularFunction at position (line 3, col 9)

function functionWithParams(param1, param2) {
    return param1 + param2;
} // functionWithParams at position (line 5, col 9)

const arrowFunction = () => {}; // arrowFunction at position (line 9, col 6)

const arrowWithParams = (x, y) => x + y; // arrowWithParams at position (line 11, col 6)

const asyncArrowFunction = async () => {
    return "async result";
}; // asyncArrowFunction at position (line 13, col 6)

async function asyncRegularFunction() {
    return "async";
} // asyncRegularFunction at position (line 17, col 15)

class MyClass {
    constructor(value) {
        this.value = value;
    } // constructor at position (line 22, col 4)
    
    method() {
        return this.value;
    } // method at position (line 26, col 4)
    
    static staticMethod() {
        return "static";
    } // staticMethod at position (line 30, col 11)
    
    get getter() {
        return this.value;
    } // getter at position (line 34, col 8)
    
    set setter(value) {
        this.value = value;
    } // setter at position (line 38, col 8)
}

const MyObject = {
    property: 42,           // property at position (line 44, col 4)
    method: function() {    // method at position (line 45, col 4)
        return this.property;
    },
    arrowMethod: () => {    // arrowMethod at position (line 48, col 4)
        return 42;
    }
};

const varConstant = 42; // varConstant at position (line 53, col 6)

let varVariable = "hello"; // varVariable at position (line 55, col 4)

var legacyVar = true; // legacyVar at position (line 57, col 4)

// Export statements
export function exportedFunction() {} // exportedFunction at position (line 60, col 16)

export const exportedConst = 42; // exportedConst at position (line 62, col 13)

export default function defaultExport() {} // defaultExport at position (line 64, col 24)

// Import statement (for testing)
// import { something } from './module'; // something would be at import position

function* generatorFunction() {
    yield 1;
    yield 2;
} // generatorFunction at position (line 69, col 10)
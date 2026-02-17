import { validateDSL } from '../../src/agent/dsl/validator.js';

describe('DSL Validator', () => {
  describe('valid programs', () => {
    test('simple variable assignment and return', () => {
      const result = validateDSL('const x = 42; return x;');
      expect(result.valid).toBe(true);
      expect(result.errors).toHaveLength(0);
    });

    test('function calls', () => {
      const result = validateDSL('const r = search("query"); return r;');
      expect(result.valid).toBe(true);
    });

    test('arrow function callback', () => {
      const result = validateDSL('const fn = (x) => x * 2; return fn(21);');
      expect(result.valid).toBe(true);
    });

    test('for...of loop', () => {
      const result = validateDSL(`
        const items = [1, 2, 3];
        const results = [];
        for (const item of items) {
          results.push(item * 2);
        }
        return results;
      `);
      expect(result.valid).toBe(true);
    });

    test('for loop with index', () => {
      const result = validateDSL(`
        const items = [1, 2, 3];
        for (let i = 0; i < items.length; i++) {
          items[i] = items[i] * 2;
        }
        return items;
      `);
      expect(result.valid).toBe(true);
    });

    test('if/else', () => {
      const result = validateDSL(`
        const x = 10;
        if (x > 5) {
          return "big";
        } else {
          return "small";
        }
      `);
      expect(result.valid).toBe(true);
    });

    test('template literals', () => {
      const result = validateDSL('const name = "world"; return `hello ${name}`;');
      expect(result.valid).toBe(true);
    });

    test('object and array literals', () => {
      const result = validateDSL(`
        const obj = { a: 1, b: [2, 3] };
        return obj;
      `);
      expect(result.valid).toBe(true);
    });

    test('member expression access', () => {
      const result = validateDSL('const arr = [1,2,3]; return arr.length;');
      expect(result.valid).toBe(true);
    });

    test('spread element', () => {
      const result = validateDSL('const a = [1,2]; const b = [...a, 3]; return b;');
      expect(result.valid).toBe(true);
    });

    test('ternary expression', () => {
      const result = validateDSL('const x = 10; return x > 5 ? "big" : "small";');
      expect(result.valid).toBe(true);
    });

    test('for...in loop', () => {
      const result = validateDSL(`
        const obj = { a: 1, b: 2, c: 3 };
        const keys = [];
        for (const key in obj) {
          keys.push(key);
        }
        return keys;
      `);
      expect(result.valid).toBe(true);
    });

    test('switch statement', () => {
      const result = validateDSL(`
        const priority = "high";
        let target;
        switch (priority) {
          case "high":
            target = 60;
            break;
          case "low":
            target = 240;
            break;
          default:
            target = 120;
        }
        return target;
      `);
      expect(result.valid).toBe(true);
    });

    test('function declaration', () => {
      const result = validateDSL(`
        function double(x) { return x * 2; }
        return double(21);
      `);
      expect(result.valid).toBe(true);
    });

    test('tagged template literal', () => {
      const result = validateDSL(`
        const result = tag\`hello \${"world"}\`;
        return result;
      `);
      expect(result.valid).toBe(true);
    });

    test('new Date for date manipulation', () => {
      const result = validateDSL(`
        const d = new Date();
        const start = new Date("2024-01-01");
        const diff = d - start;
        return diff;
      `);
      expect(result.valid).toBe(true);
    });

    test('typical DSL program', () => {
      const result = validateDSL(`
        const results = search("API endpoints", "./src");
        const chunks = chunk(results, 20000);
        const extracted = map(chunks, (c) => LLM("Extract endpoints", c));
        return LLM("Organize by resource", extracted);
      `);
      expect(result.valid).toBe(true);
    });
  });

  describe('blocked constructs', () => {
    test('rejects async function', () => {
      const result = validateDSL('const fn = async (x) => x;');
      expect(result.valid).toBe(false);
      expect(result.errors[0]).toContain('Async functions are not allowed');
    });

    test('rejects await expression', () => {
      const result = validateDSL('const x = await fetch("url");');
      // This should fail at parse time since await outside async is invalid in script mode
      expect(result.valid).toBe(false);
    });

    test('rejects class declaration', () => {
      const result = validateDSL('class Foo {}');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes('ClassDeclaration') || e.includes('ClassBody'))).toBe(true);
    });

    test('allows new expression for safe constructors', () => {
      const result = validateDSL('const d = new Date();');
      expect(result.valid).toBe(true);
    });

    test('still rejects new Function (blocked identifier)', () => {
      const result = validateDSL('const fn = new Function("return 1");');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes("'Function'"))).toBe(true);
    });

    test('rejects this', () => {
      const result = validateDSL('const x = this;');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes('ThisExpression'))).toBe(true);
    });

    test('rejects eval', () => {
      const result = validateDSL('eval("alert(1)");');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes("'eval'"))).toBe(true);
    });

    test('rejects require', () => {
      const result = validateDSL('const fs = require("fs");');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes("'require'"))).toBe(true);
    });

    test('rejects process access', () => {
      const result = validateDSL('const env = process.env;');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes("'process'"))).toBe(true);
    });

    test('rejects __proto__ access', () => {
      const result = validateDSL('const x = obj.__proto__;');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes('__proto__'))).toBe(true);
    });

    test('rejects constructor access', () => {
      const result = validateDSL('const x = "".constructor;');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes('constructor'))).toBe(true);
    });

    test('rejects prototype access', () => {
      const result = validateDSL('const x = Array.prototype;');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes('prototype'))).toBe(true);
    });

    test('rejects computed __proto__ access', () => {
      const result = validateDSL('const x = obj["__proto__"];');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes("__proto__"))).toBe(true);
    });

    test('rejects import expression', () => {
      // Dynamic import
      const result = validateDSL('const m = import("fs");');
      expect(result.valid).toBe(false);
    });

    test('rejects with statement', () => {
      // 'with' is not allowed in strict mode, and our parser uses script mode
      // which should still catch it
      const result = validateDSL('with (obj) { return x; }');
      // This may fail at parse time or validation time depending on strict mode
      expect(result.valid).toBe(false);
    });

    test('rejects globalThis', () => {
      const result = validateDSL('const x = globalThis;');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes('globalThis'))).toBe(true);
    });

    test('rejects Function constructor', () => {
      const result = validateDSL('const fn = Function("return 1");');
      expect(result.valid).toBe(false);
      expect(result.errors.some(e => e.includes("'Function'"))).toBe(true);
    });

    test('rejects generator function', () => {
      const result = validateDSL('const gen = function* () { yield 1; };');
      expect(result.valid).toBe(false);
    });

    test('allows regex literals', () => {
      const result = validateDSL('const x = /pattern/.test("hello");');
      expect(result.valid).toBe(true);
    });

    test('allows regex in replace', () => {
      const result = validateDSL('const s = "hello".replace(/h/, "H");');
      expect(result.valid).toBe(true);
    });

    test('allows regex with flags', () => {
      const result = validateDSL('const s = "Hello".replace(/hello/gi, "world");');
      expect(result.valid).toBe(true);
    });
  });

  describe('syntax errors', () => {
    test('reports syntax errors', () => {
      const result = validateDSL('const x = ;');
      expect(result.valid).toBe(false);
      expect(result.errors[0]).toContain('Syntax error');
    });

    test('reports unclosed brackets', () => {
      const result = validateDSL('const x = [1, 2, 3');
      expect(result.valid).toBe(false);
      expect(result.errors[0]).toContain('Syntax error');
    });
  });
});

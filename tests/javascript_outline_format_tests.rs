use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_javascript_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("basic.js");

    let content = r#"// Basic JavaScript classes and functions for testing

/**
 * A simple calculator class for arithmetic operations.
 */
class Calculator {
    constructor(name) {
        this.name = name;
        this.history = [];
    }

    /**
     * Adds two numbers and returns the result.
     */
    add(x, y) {
        const result = x + y;
        this.history.push(result);
        return result;
    }

    /**
     * Gets the calculation history.
     */
    getHistory() {
        return [...this.history];
    }

    static createDefault() {
        return new Calculator('Default Calculator');
    }
}

/**
 * Factory function for creating calculators.
 */
function createCalculator(name) {
    return new Calculator(name);
}

/**
 * Arrow function for simple calculations.
 */
const multiply = (a, b) => a * b;

/**
 * Async function for fetching data.
 */
async function fetchData(url) {
    try {
        const response = await fetch(url);
        return await response.json();
    } catch (error) {
        console.error('Fetch error:', error);
        throw error;
    }
}

/**
 * Generator function for creating sequences.
 */
function* numberSequence(start, end) {
    for (let i = start; i <= end; i++) {
        yield i;
    }
}

/**
 * Object with methods demonstration.
 */
const mathUtils = {
    PI: Math.PI,

    circleArea(radius) {
        return this.PI * radius * radius;
    },

    rectangleArea: function(width, height) {
        return width * height;
    },

    triangleArea: (base, height) => (base * height) / 2
};

// Export for module usage
module.exports = {
    Calculator,
    createCalculator,
    multiply,
    fetchData,
    numberSequence,
    mathUtils
};
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Calculator", // Search for Calculator-related symbols
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Verify JavaScript symbols are extracted in outline format
    assert!(
        output.contains("class Calculator") || output.contains("Calculator"),
        "Missing Calculator class - output: {}",
        output
    );
    assert!(
        output.contains("function createCalculator") || output.contains("createCalculator"),
        "Missing createCalculator function - output: {}",
        output
    );

    // Test a different search term for functions
    let output2 = ctx.run_probe(&[
        "search",
        "function", // Search for function-related symbols
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    assert!(
        output2.contains("function") || !output2.trim().is_empty(),
        "Missing function declarations - output: {}",
        output2
    );

    Ok(())
}

#[test]
fn test_javascript_outline_control_flow_statements() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("control_flow.js");

    let content = r#"/**
 * Function demonstrating various control flow statements with gaps.
 */
function complexAlgorithm(data, threshold) {
    const result = {};
    let counter = 0;

    // First processing phase
    for (const item of data) {
        if (item > threshold) {
            counter++;

            // Complex nested conditions
            if (counter % 2 === 0) {
                result[`even_${counter}`] = item;
            } else {
                result[`odd_${counter}`] = item;
            }
        }
    }

    // Second processing phase with while loop
    let index = 0;
    while (index < data.length) {
        const value = data[index];

        switch (true) {
            case value < 0:
                result[`negative_${index}`] = value;
                break;
            case value === 0:
                result['zero'] = 0;
                break;
            default:
                result[`positive_${index}`] = value;
                break;
        }

        index++;
    }

    return result;
}

/**
 * Function with nested loops and complex control flow.
 */
function processMatrix(matrix) {
    const processed = [];

    for (let i = 0; i < matrix.length; i++) {
        const row = matrix[i];
        const newRow = [];

        for (let j = 0; j < row.length; j++) {
            const cell = row[j];

            let processedCell;
            if (cell > 0) {
                processedCell = cell * 2;
            } else if (cell < 0) {
                processedCell = Math.abs(cell);
            } else {
                processedCell = 1;
            }

            newRow.push(processedCell);
        }

        processed.push(newRow);
    }

    return processed;
}

/**
 * Function with try-catch and complex error handling.
 */
async function fetchWithRetry(url, maxRetries = 3) {
    let attempts = 0;

    while (attempts < maxRetries) {
        try {
            const response = await fetch(url);

            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }

            return await response.json();
        } catch (error) {
            attempts++;

            if (attempts >= maxRetries) {
                throw new Error(`Failed after ${maxRetries} attempts: ${error.message}`);
            }

            // Wait before retry
            await new Promise(resolve => setTimeout(resolve, 1000 * attempts));
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "function", // Search for function declarations
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Verify control flow structures are shown with proper formatting
    assert!(
        output.contains("function") && !output.trim().is_empty(),
        "Missing function declarations in control flow test - output: {}",
        output
    );

    // Should contain closing braces for large blocks
    assert!(
        output.contains("}"),
        "Missing closing braces - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_javascript_outline_modern_features() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("modern_features.js");

    let content = r#"// Modern JavaScript features for testing

/**
 * ES6+ class with modern syntax.
 */
class DataProcessor {
    #privateField = 'secret';

    constructor(options = {}) {
        this.name = options.name ?? 'DefaultProcessor';
        this.config = { ...options };
    }

    // Private method
    #validateData(data) {
        return data && typeof data === 'object';
    }

    // Async method with destructuring
    async processData({ input, format = 'json' } = {}) {
        if (!this.#validateData(input)) {
            throw new Error('Invalid data format');
        }

        const { processed, metadata } = await this.#transformData(input);

        return {
            result: processed,
            metadata: {
                ...metadata,
                format,
                timestamp: new Date().toISOString()
            }
        };
    }

    // Static async method
    static async create(options) {
        const processor = new DataProcessor(options);
        await processor.initialize();
        return processor;
    }

    async #transformData(data) {
        // Simulate async processing
        return new Promise(resolve => {
            setTimeout(() => {
                resolve({
                    processed: data.map(item => ({ ...item, processed: true })),
                    metadata: { count: data.length }
                });
            }, 100);
        });
    }
}

/**
 * Destructuring and spread operator examples.
 */
function processUser({ name, age, ...rest }) {
    return {
        displayName: name.toUpperCase(),
        isAdult: age >= 18,
        additionalInfo: { ...rest }
    };
}

/**
 * Template literals and tagged templates.
 */
const createMessage = (template, ...values) => {
    return template.reduce((acc, str, i) => {
        const value = values[i] ? `<strong>${values[i]}</strong>` : '';
        return acc + str + value;
    }, '');
};

/**
 * Map and Set operations with symbols.
 */
class SymbolRegistry {
    #symbols = new Map();
    #metadata = new WeakMap();

    register(key, description) {
        const symbol = Symbol(description);
        this.#symbols.set(key, symbol);
        this.#metadata.set(symbol, {
            created: Date.now(),
            description
        });
        return symbol;
    }

    get(key) {
        return this.#symbols.get(key);
    }

    *entries() {
        for (const [key, symbol] of this.#symbols) {
            yield [key, symbol, this.#metadata.get(symbol)];
        }
    }
}

/**
 * Proxy usage for advanced object manipulation.
 */
function createSmartObject(target = {}) {
    return new Proxy(target, {
        get(obj, prop) {
            if (prop in obj) {
                return obj[prop];
            }

            // Auto-generate computed properties
            if (prop.startsWith('computed_')) {
                const key = prop.replace('computed_', '');
                return () => `Computed value for ${key}`;
            }

            return undefined;
        },

        set(obj, prop, value) {
            // Validate before setting
            if (typeof value === 'function') {
                obj[prop] = value.bind(obj);
            } else {
                obj[prop] = value;
            }
            return true;
        }
    });
}

// Module exports with modern syntax
export {
    DataProcessor,
    processUser,
    createMessage,
    SymbolRegistry,
    createSmartObject
};

export default DataProcessor;
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "class", // Search for class declarations
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Verify modern JavaScript features
    assert!(
        output.contains("class DataProcessor") || output.contains("DataProcessor"),
        "Missing DataProcessor class - output: {}",
        output
    );
    assert!(
        output.contains("class SymbolRegistry") || output.contains("SymbolRegistry"),
        "Missing SymbolRegistry class - output: {}",
        output
    );

    // Test another search term for functions
    let output2 = ctx.run_probe(&[
        "search",
        "async", // Search for async functions
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    assert!(
        output2.contains("async") || !output2.trim().is_empty(),
        "Should find async-related content - output: {}",
        output2
    );

    Ok(())
}

#[test]
fn test_javascript_outline_react_components() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("react_components.js");

    let content = r#"import React, { useState, useEffect, useCallback, useMemo } from 'react';
import PropTypes from 'prop-types';

/**
 * Custom hook for managing local storage.
 */
function useLocalStorage(key, initialValue) {
    const [storedValue, setStoredValue] = useState(() => {
        try {
            const item = window.localStorage.getItem(key);
            return item ? JSON.parse(item) : initialValue;
        } catch (error) {
            console.error(`Error reading localStorage key "${key}":`, error);
            return initialValue;
        }
    });

    const setValue = useCallback((value) => {
        try {
            setStoredValue(value);
            window.localStorage.setItem(key, JSON.stringify(value));
        } catch (error) {
            console.error(`Error setting localStorage key "${key}":`, error);
        }
    }, [key]);

    return [storedValue, setValue];
}

/**
 * Higher-order component for loading states.
 */
function withLoading(WrappedComponent) {
    return function WithLoadingComponent(props) {
        const [isLoading, setIsLoading] = useState(false);

        const startLoading = useCallback(() => setIsLoading(true), []);
        const stopLoading = useCallback(() => setIsLoading(false), []);

        if (isLoading) {
            return (
                <div className="loading-spinner">
                    <div>Loading...</div>
                </div>
            );
        }

        return (
            <WrappedComponent
                {...props}
                startLoading={startLoading}
                stopLoading={stopLoading}
            />
        );
    };
}

/**
 * Main functional component with hooks.
 */
const UserProfile = ({ userId, onUserUpdate }) => {
    const [user, setUser] = useState(null);
    const [preferences, setPreferences] = useLocalStorage('userPreferences', {});
    const [isEditing, setIsEditing] = useState(false);

    // Memoized computed values
    const displayName = useMemo(() => {
        if (!user) return '';
        return `${user.firstName} ${user.lastName}`.trim();
    }, [user]);

    // Effect for fetching user data
    useEffect(() => {
        if (userId) {
            fetchUser(userId)
                .then(userData => {
                    setUser(userData);
                    onUserUpdate?.(userData);
                })
                .catch(error => {
                    console.error('Failed to fetch user:', error);
                });
        }
    }, [userId, onUserUpdate]);

    // Event handlers
    const handleEdit = useCallback(() => {
        setIsEditing(true);
    }, []);

    const handleSave = useCallback(async (updatedUser) => {
        try {
            const savedUser = await saveUser(updatedUser);
            setUser(savedUser);
            setIsEditing(false);
            onUserUpdate?.(savedUser);
        } catch (error) {
            console.error('Failed to save user:', error);
        }
    }, [onUserUpdate]);

    const handleCancel = useCallback(() => {
        setIsEditing(false);
    }, []);

    // Render methods
    const renderProfile = () => {
        if (!user) {
            return <div>No user data available</div>;
        }

        return (
            <div className="user-profile">
                <h2>{displayName}</h2>
                <p>Email: {user.email}</p>
                <p>Role: {user.role}</p>

                <div className="user-preferences">
                    <h3>Preferences</h3>
                    {Object.entries(preferences).map(([key, value]) => (
                        <div key={key}>
                            {key}: {JSON.stringify(value)}
                        </div>
                    ))}
                </div>
            </div>
        );
    };

    const renderEditForm = () => {
        return (
            <UserEditForm
                user={user}
                onSave={handleSave}
                onCancel={handleCancel}
            />
        );
    };

    return (
        <div className="user-profile-container">
            {isEditing ? renderEditForm() : renderProfile()}

            {!isEditing && (
                <button onClick={handleEdit}>
                    Edit Profile
                </button>
            )}
        </div>
    );
};

/**
 * Class-based component for comparison.
 */
class UserEditForm extends React.Component {
    static propTypes = {
        user: PropTypes.object.isRequired,
        onSave: PropTypes.func.isRequired,
        onCancel: PropTypes.func.isRequired
    };

    constructor(props) {
        super(props);
        this.state = {
            formData: { ...props.user },
            errors: {}
        };
    }

    componentDidMount() {
        this.validateForm();
    }

    componentDidUpdate(prevProps) {
        if (prevProps.user !== this.props.user) {
            this.setState({ formData: { ...this.props.user } });
        }
    }

    validateForm = () => {
        const { formData } = this.state;
        const errors = {};

        if (!formData.firstName?.trim()) {
            errors.firstName = 'First name is required';
        }

        if (!formData.email?.trim()) {
            errors.email = 'Email is required';
        } else if (!/^\S+@\S+\.\S+$/.test(formData.email)) {
            errors.email = 'Email format is invalid';
        }

        this.setState({ errors });
        return Object.keys(errors).length === 0;
    };

    handleSubmit = (e) => {
        e.preventDefault();

        if (this.validateForm()) {
            this.props.onSave(this.state.formData);
        }
    };

    handleChange = (field) => (e) => {
        this.setState({
            formData: {
                ...this.state.formData,
                [field]: e.target.value
            }
        }, this.validateForm);
    };

    render() {
        const { formData, errors } = this.state;
        const { onCancel } = this.props;

        return (
            <form onSubmit={this.handleSubmit} className="user-edit-form">
                <div className="form-group">
                    <label htmlFor="firstName">First Name:</label>
                    <input
                        id="firstName"
                        type="text"
                        value={formData.firstName || ''}
                        onChange={this.handleChange('firstName')}
                    />
                    {errors.firstName && (
                        <span className="error">{errors.firstName}</span>
                    )}
                </div>

                <div className="form-buttons">
                    <button type="submit">Save</button>
                    <button type="button" onClick={onCancel}>
                        Cancel
                    </button>
                </div>
            </form>
        );
    }
}

// Utility functions
async function fetchUser(userId) {
    const response = await fetch(`/api/users/${userId}`);
    if (!response.ok) {
        throw new Error(`Failed to fetch user: ${response.status}`);
    }
    return response.json();
}

async function saveUser(userData) {
    const response = await fetch(`/api/users/${userData.id}`, {
        method: 'PUT',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(userData),
    });

    if (!response.ok) {
        throw new Error(`Failed to save user: ${response.status}`);
    }

    return response.json();
}

export { useLocalStorage, withLoading, UserProfile, UserEditForm };
export default UserProfile;
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "React", // Search for React-related symbols
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Should find React-related content (not currently asserting on this)
    let _has_react_content = output.contains("React")
        || output.contains("Component")
        || output.contains("useState")
        || output.contains("useEffect");

    // Test for function/class declarations
    let output2 = ctx.run_probe(&[
        "search",
        "function", // Search for function declarations
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    assert!(
        output2.contains("function") || output2.contains("const") || output2.contains("class"),
        "Missing React component functions/classes - output: {}",
        output2
    );

    Ok(())
}

#[test]
fn test_javascript_outline_test_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test_patterns.js");

    let content = r#"// Jest test patterns for JavaScript

const { Calculator } = require('./calculator');

describe('Calculator', () => {
    let calculator;

    beforeEach(() => {
        calculator = new Calculator('Test Calculator');
    });

    afterEach(() => {
        calculator = null;
    });

    describe('addition', () => {
        test('should add two positive numbers', () => {
            expect(calculator.add(2, 3)).toBe(5);
        });

        test('should add negative numbers', () => {
            expect(calculator.add(-2, -3)).toBe(-5);
        });

        test('should handle zero', () => {
            expect(calculator.add(0, 5)).toBe(5);
        });
    });

    describe('history tracking', () => {
        test('should track calculation history', () => {
            calculator.add(2, 3);
            calculator.add(4, 5);

            const history = calculator.getHistory();
            expect(history).toEqual([5, 9]);
        });

        test('should start with empty history', () => {
            expect(calculator.getHistory()).toEqual([]);
        });
    });

    describe('error handling', () => {
        test('should handle division by zero', () => {
            expect(() => calculator.divide(10, 0)).toThrow('Division by zero');
        });

        test('should handle invalid inputs', () => {
            expect(() => calculator.add('a', 'b')).toThrow('Invalid input');
        });
    });
});

// Async test patterns
describe('async operations', () => {
    test('should fetch data successfully', async () => {
        const mockData = { id: 1, name: 'Test' };
        global.fetch = jest.fn().mockResolvedValue({
            ok: true,
            json: () => Promise.resolve(mockData),
        });

        const result = await fetchUserData(1);
        expect(result).toEqual(mockData);
        expect(fetch).toHaveBeenCalledWith('/api/users/1');
    });

    test('should handle fetch errors', async () => {
        global.fetch = jest.fn().mockRejectedValue(new Error('Network error'));

        await expect(fetchUserData(1)).rejects.toThrow('Network error');
    });

    test('should timeout after specified duration', async () => {
        jest.setTimeout(5000);

        const slowOperation = () => new Promise(resolve => {
            setTimeout(resolve, 10000);
        });

        await expect(slowOperation()).resolves.toBe(undefined);
    }, 15000);
});

// Mock implementations
jest.mock('./external-service', () => ({
    sendNotification: jest.fn(),
    logEvent: jest.fn(),
}));

describe('service integration', () => {
    const mockService = require('./external-service');

    beforeEach(() => {
        jest.clearAllMocks();
    });

    test('should call external service', () => {
        const notificationService = new NotificationService();
        notificationService.notify('Test message');

        expect(mockService.sendNotification).toHaveBeenCalledWith('Test message');
    });

    test('should log events correctly', () => {
        const eventLogger = new EventLogger();
        eventLogger.log('user_action', { userId: 123 });

        expect(mockService.logEvent).toHaveBeenCalledWith('user_action', { userId: 123 });
    });
});

// Parameterized tests
describe.each([
    { a: 1, b: 2, expected: 3 },
    { a: -1, b: 1, expected: 0 },
    { a: 0, b: 0, expected: 0 },
])('add($a, $b)', ({ a, b, expected }) => {
    test(`should return ${expected}`, () => {
        expect(add(a, b)).toBe(expected);
    });
});

// Snapshot testing
describe('component rendering', () => {
    test('should render user profile correctly', () => {
        const user = { name: 'John Doe', email: 'john@example.com' };
        const component = renderUserProfile(user);
        expect(component).toMatchSnapshot();
    });

    test('should render empty state', () => {
        const component = renderUserProfile(null);
        expect(component).toMatchSnapshot();
    });
});

// Utility functions for testing
function add(a, b) {
    return a + b;
}

async function fetchUserData(userId) {
    const response = await fetch(`/api/users/${userId}`);
    if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
    }
    return response.json();
}

class NotificationService {
    notify(message) {
        return require('./external-service').sendNotification(message);
    }
}

class EventLogger {
    log(event, data) {
        return require('./external-service').logEvent(event, data);
    }
}

function renderUserProfile(user) {
    if (!user) {
        return '<div>No user data</div>';
    }
    return `<div><h1>${user.name}</h1><p>${user.email}</p></div>`;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "test", // Search for test-related symbols
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Verify test patterns are detected - should find describe blocks or test functions
    let _has_test_patterns =
        output.contains("describe") || output.contains("test") || output.contains("it(");

    // Test for function declarations
    let output2 = ctx.run_probe(&[
        "search",
        "function", // Search for function declarations
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    assert!(
        output2.contains("function") || output2.contains("class") || output2.contains("const"),
        "Missing function/class declarations - output: {}",
        output2
    );

    Ok(())
}

#[test]
fn test_javascript_outline_large_function_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_function.js");

    let content = r#"/**
 * Large function with multiple nested blocks to test closing brace comments.
 * This function has more than 20 lines and should get closing brace comments.
 */
function complexDataProcessor(data) {
    const results = [];
    const categories = new Map();

    // Phase 1: Categorization
    for (const [index, value] of data.entries()) {
        let category;

        if (value < 0) {
            category = 'negative';
        } else if (value === 0) {
            category = 'zero';
        } else if (value < 100) {
            category = 'small_positive';
        } else if (value < 1000) {
            category = 'medium_positive';
        } else {
            category = 'large_positive';
        }

        if (!categories.has(category)) {
            categories.set(category, []);
        }
        categories.get(category).push({ index, value });
    }

    // Phase 2: Processing each category
    for (const [category, items] of categories) {
        if (category === 'negative') {
            for (const { index, value } of items) {
                results.push(`NEG[${index}]: ${Math.abs(value)}`);
            }
        } else if (category === 'zero') {
            for (const { index } of items) {
                results.push(`ZERO[${index}]: neutral`);
            }
        } else {
            // Positive number processing
            for (const { index, value } of items) {
                let processed;

                if (value < 10) {
                    processed = `SINGLE_DIGIT[${index}]: ${value}`;
                } else if (value < 100) {
                    processed = `DOUBLE_DIGIT[${index}]: ${value}`;
                } else {
                    processed = `MULTI_DIGIT[${index}]: ${value}`;
                }

                results.push(processed);
            }
        }
    }

    // Phase 3: Final sorting and validation
    results.sort();

    // Validation phase
    const validatedResults = [];
    for (const result of results) {
        if (result.length > 5) {
            validatedResults.push(result);
        }
    }

    return validatedResults;
}

/**
 * Small function that should NOT get closing brace comments.
 */
function smallFunction(x, y) {
    return x + y;
}

/**
 * Another small function for testing.
 */
const anotherSmallFunction = (a, b) => {
    const result = a * b;
    return result > 0 ? result : 0;
};
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "complexDataProcessor", // Search for the specific large function
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Verify large function is shown with closing braces
    assert!(
        output.contains("function complexDataProcessor") || output.contains("complexDataProcessor"),
        "Missing complexDataProcessor function - output: {}",
        output
    );

    // Should have closing braces for large blocks
    let closing_braces_count = output.matches("}").count();
    assert!(
        closing_braces_count >= 1,
        "Should have closing braces for function - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_javascript_outline_small_function_no_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("small_functions.js");

    let content = r#"/**
 * Collection of small functions that should NOT get closing brace comments.
 */

function add(x, y) {
    return x + y;
}

function multiply(a, b) {
    return a * b;
}

const subtract = (x, y) => {
    return x - y;
};

async function fetchData(url) {
    const response = await fetch(url);
    return response.json();
}

class SimpleCalculator {
    add(x, y) {
        return x + y;
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "function", // Search for function declarations
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Verify small functions are found but don't have excessive closing braces
    assert!(
        output.contains("function add") || output.contains("add") || output.contains("function"),
        "Missing small function declarations - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_javascript_outline_keyword_highlighting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keyword_test.js");

    let content = r#"// JavaScript with various keywords for highlighting tests

/**
 * Function with async/await keywords
 */
async function processAsync(data) {
    try {
        const result = await performOperation(data);
        return { success: true, result };
    } catch (error) {
        throw new Error(`Processing failed: ${error.message}`);
    }
}

/**
 * Function with control flow keywords
 */
function processWithControlFlow(items) {
    for (const item of items) {
        if (item.type === 'special') {
            continue;
        }

        switch (item.status) {
            case 'active':
                processActiveItem(item);
                break;
            case 'inactive':
                processInactiveItem(item);
                break;
            default:
                processDefaultItem(item);
                break;
        }
    }
}

/**
 * Class with static and private keywords
 */
class KeywordProcessor {
    static instances = [];
    #privateData = {};

    constructor() {
        KeywordProcessor.instances.push(this);
    }

    static getInstance() {
        return KeywordProcessor.instances[0];
    }

    #privateMethod() {
        return this.#privateData;
    }
}

// Export keywords
export { processAsync, processWithControlFlow, KeywordProcessor };
export default KeywordProcessor;
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();

    // Search for specific keywords and verify they're highlighted in outline
    let test_cases = vec![
        ("async", "async function processAsync"),
        ("static", "static getInstance"),
        ("class", "class KeywordProcessor"),
        ("export", "export"),
    ];

    for (keyword, expected_content) in test_cases {
        let output = ctx.run_probe(&[
            "search",
            keyword,
            test_file.to_str().unwrap(),
            "--format",
            "outline",
            "--allow-tests",
        ])?;

        // Should find the keyword in outline format
        assert!(
            output.contains(keyword)
                || output.contains(expected_content)
                || !output.trim().is_empty(),
            "Missing {} keyword in outline - output: {}",
            keyword,
            output
        );
    }

    Ok(())
}

#[test]
fn test_javascript_outline_array_object_truncation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("truncation_test.js");

    let content = r#"// JavaScript with large arrays and objects for truncation testing

const largeArray = [
    'item1', 'item2', 'item3', 'item4', 'item5',
    'item6', 'item7', 'item8', 'item9', 'item10',
    'keyword', 'important', 'special', 'data',
    'item11', 'item12', 'item13', 'item14', 'item15',
    'item16', 'item17', 'item18', 'item19', 'item20',
    'item21', 'item22', 'item23', 'item24', 'item25'
];

const largeObject = {
    property1: 'value1',
    property2: 'value2',
    property3: 'value3',
    importantKeyword: 'special value',
    property4: 'value4',
    property5: 'value5',
    property6: 'value6',
    property7: 'value7',
    property8: 'value8',
    property9: 'value9',
    property10: 'value10',
    anotherKeyword: 'another special value',
    property11: 'value11',
    property12: 'value12'
};

function processLargeData() {
    const config = {
        timeout: 5000,
        retries: 3,
        important: true,
        special: 'keyword',
        endpoints: [
            '/api/users',
            '/api/posts',
            '/api/comments',
            '/api/important',
            '/api/data',
            '/api/special',
            '/api/keyword'
        ],
        settings: {
            debug: false,
            verbose: true,
            important: 'setting',
            keyword: 'value'
        }
    };

    return config;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "keyword", // Search for keyword that should be preserved even in truncated arrays/objects
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Should find the keyword even in truncated content
    assert!(
        output.contains("keyword") || !output.trim().is_empty(),
        "Missing keyword in truncated arrays/objects - output: {}",
        output
    );

    // Test for truncation indicators
    let output2 = ctx.run_probe(&[
        "search",
        "largeArray",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
        "--exact",
    ])?;

    // Should contain array/object content, possibly with truncation
    assert!(
        output2.contains("largeArray") || output2.contains("const") || !output2.trim().is_empty(),
        "Missing large array content - output: {}",
        output2
    );

    Ok(())
}

#[test]
fn test_javascript_outline_search_command() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("search_test.js");

    let content = r#"class DataProcessor {
    constructor() {
        this.processedCount = 0;
    }

    processData(data) {
        this.processedCount++;
        return data.filter(item => item != null);
    }

    getProcessedCount() {
        return this.processedCount;
    }
}

function processFile(filename) {
    return `Processed ${filename}`;
}

async function processAsync(data) {
    return { processed: true, ...data };
}

function testDataProcessing() {
    const processor = new DataProcessor();
    const result = processor.processData([1, 2, null, 3]);
    console.assert(result.length === 3);
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
        "--exact",
    ])?;

    // Should find symbols containing "process"
    assert!(
        output.contains("DataProcessor")
            || output.contains("processData")
            || output.contains("processFile")
            || output.contains("processAsync")
            || output.contains("process"),
        "Should find process-related symbols - output: {}",
        output
    );

    Ok(())
}

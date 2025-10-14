// Test fixture for C++ tree-sitter position validation
// Line numbers and symbol positions are tested precisely

#include <iostream>
#include <vector>
#include <memory>

void simple_function() {} // simple_function at position (line 8, col 5)

int function_with_return() { // function_with_return at position (line 10, col 4)
    return 42;
}

void function_with_params(int param1, const std::string& param2) { // function_with_params at position (line 14, col 5)
    std::cout << param1 << " " << param2 << std::endl;
}

namespace MyNamespace { // MyNamespace at position (line 18, col 10)
    void namespaced_function() {} // namespaced_function at position (line 19, col 9)
    
    class NamespacedClass { // NamespacedClass at position (line 21, col 10)
        public:
            void method() {} // method at position (line 23, col 17)
    };
}

class SimpleClass { // SimpleClass at position (line 27, col 6)
private:
    int private_field; // private_field at position (line 29, col 8)
    
public:
    SimpleClass() : private_field(0) {} // SimpleClass at position (line 32, col 4)
    
    SimpleClass(int value) : private_field(value) {} // SimpleClass at position (line 34, col 4)
    
    ~SimpleClass() {} // SimpleClass at position (line 36, col 5) (destructor)
    
    void method() { // method at position (line 38, col 9)
        std::cout << "Method called" << std::endl;
    }
    
    virtual void virtual_method() { // virtual_method at position (line 42, col 17)
        std::cout << "Virtual method" << std::endl;
    }
    
    static void static_method() { // static_method at position (line 46, col 16)
        std::cout << "Static method" << std::endl;
    }
    
    int get_field() const { // get_field at position (line 50, col 8)
        return private_field;
    }
    
    void set_field(int value) { // set_field at position (line 54, col 9)
        private_field = value;
    }
};

class DerivedClass : public SimpleClass { // DerivedClass at position (line 59, col 6)
public:
    DerivedClass() : SimpleClass() {} // DerivedClass at position (line 61, col 4)
    
    void virtual_method() override { // virtual_method at position (line 63, col 9)
        std::cout << "Overridden virtual method" << std::endl;
    }
    
    void new_method() { // new_method at position (line 67, col 9)
        std::cout << "New method in derived class" << std::endl;
    }
};

template<typename T>
class TemplateClass { // TemplateClass at position (line 73, col 6)
private:
    T value; // value at position (line 75, col 6)
    
public:
    TemplateClass(T val) : value(val) {} // TemplateClass at position (line 78, col 4)
    
    T get_value() const { // get_value at position (line 80, col 6)
        return value;
    }
    
    void set_value(T val) { // set_value at position (line 84, col 9)
        value = val;
    }
};

template<typename T>
void template_function(T value) { // template_function at position (line 90, col 5)
    std::cout << "Template function: " << value << std::endl;
}

struct SimpleStruct { // SimpleStruct at position (line 94, col 7)
    int field1;        // field1 at position (line 95, col 8)
    std::string field2; // field2 at position (line 96, col 16)
    
    SimpleStruct() : field1(0), field2("") {} // SimpleStruct at position (line 98, col 4)
    
    void struct_method() { // struct_method at position (line 100, col 9)
        std::cout << "Struct method" << std::endl;
    }
};

enum Color { // Color at position (line 105, col 5)
    RED,     // RED at position (line 106, col 4)
    GREEN,   // GREEN at position (line 107, col 4)
    BLUE     // BLUE at position (line 108, col 4)
};

enum class StrongEnum { // StrongEnum at position (line 111, col 11)
    VALUE1,  // VALUE1 at position (line 112, col 4)
    VALUE2,  // VALUE2 at position (line 113, col 4)
    VALUE3   // VALUE3 at position (line 114, col 4)
};

// Function overloading
void overloaded_function() { // overloaded_function at position (line 118, col 5)
    std::cout << "No parameters" << std::endl;
}

void overloaded_function(int param) { // overloaded_function at position (line 122, col 5)
    std::cout << "Int parameter: " << param << std::endl;
}

void overloaded_function(const std::string& param) { // overloaded_function at position (line 126, col 5)
    std::cout << "String parameter: " << param << std::endl;
}

// Operator overloading
class Point { // Point at position (line 131, col 6)
public:
    int x, y; // x at position (line 133, col 8), y at position (line 133, col 11)
    
    Point(int x = 0, int y = 0) : x(x), y(y) {} // Point at position (line 135, col 4)
    
    Point operator+(const Point& other) const { // operator+ at position (line 137, col 10)
        return Point(x + other.x, y + other.y);
    }
    
    bool operator==(const Point& other) const { // operator== at position (line 141, col 9)
        return x == other.x && y == other.y;
    }
};

// Global variables
int global_var = 42;        // global_var at position (line 147, col 4)
const int const_var = 100;  // const_var at position (line 148, col 10)
static int static_var = 0;  // static_var at position (line 149, col 11)

int main() { // main at position (line 151, col 4)
    return 0;
}
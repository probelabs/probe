// Test fixture for Java tree-sitter position validation
// Line numbers and symbol positions are tested precisely

public class JavaPositions { // JavaPositions at position (line 3, col 13)
    
    private int privateField; // privateField at position (line 5, col 16)
    public String publicField; // publicField at position (line 6, col 18)
    protected boolean protectedField; // protectedField at position (line 7, col 22)
    
    public JavaPositions() { // JavaPositions at position (line 9, col 11)
        this.privateField = 0;
    }
    
    public JavaPositions(int value) { // JavaPositions at position (line 13, col 11)
        this.privateField = value;
    }
    
    public void simpleMethod() { // simpleMethod at position (line 17, col 16)
        // simple method implementation
    }
    
    public String methodWithParams(int param1, String param2) { // methodWithParams at position (line 21, col 18)
        return param1 + param2;
    }
    
    public static void staticMethod() { // staticMethod at position (line 25, col 23)
        System.out.println("Static method");
    }
    
    private void privateMethod() { // privateMethod at position (line 29, col 17)
        // private implementation
    }
    
    protected void protectedMethod() { // protectedMethod at position (line 33, col 19)
        // protected implementation
    }
    
    public final void finalMethod() { // finalMethod at position (line 37, col 22)
        // final method implementation
    }
    
    public abstract void abstractMethod(); // abstractMethod at position (line 41, col 25)
    
    // Getter and setter methods
    public int getPrivateField() { // getPrivateField at position (line 44, col 15)
        return this.privateField;
    }
    
    public void setPrivateField(int value) { // setPrivateField at position (line 48, col 16)
        this.privateField = value;
    }
}

interface MyInterface { // MyInterface at position (line 53, col 10)
    void interfaceMethod(); // interfaceMethod at position (line 54, col 9)
    
    default void defaultMethod() { // defaultMethod at position (line 56, col 17)
        System.out.println("Default interface method");
    }
    
    static void staticInterfaceMethod() { // staticInterfaceMethod at position (line 60, col 16)
        System.out.println("Static interface method");
    }
}

abstract class AbstractClass { // AbstractClass at position (line 65, col 15)
    protected int value; // value at position (line 66, col 18)
    
    public AbstractClass(int value) { // AbstractClass at position (line 68, col 11)
        this.value = value;
    }
    
    public abstract void abstractMethod(); // abstractMethod at position (line 72, col 25)
    
    public void concreteMethod() { // concreteMethod at position (line 74, col 16)
        System.out.println("Concrete method");
    }
}

class ConcreteClass extends AbstractClass implements MyInterface { // ConcreteClass at position (line 79, col 6)
    
    public ConcreteClass(int value) { // ConcreteClass at position (line 81, col 11)
        super(value);
    }
    
    @Override
    public void abstractMethod() { // abstractMethod at position (line 86, col 16)
        System.out.println("Implemented abstract method");
    }
    
    @Override
    public void interfaceMethod() { // interfaceMethod at position (line 91, col 16)
        System.out.println("Implemented interface method");
    }
}

enum Color { // Color at position (line 96, col 5)
    RED,    // RED at position (line 97, col 4)
    GREEN,  // GREEN at position (line 98, col 4)
    BLUE;   // BLUE at position (line 99, col 4)
    
    public String getColorName() { // getColorName at position (line 101, col 18)
        return this.name().toLowerCase();
    }
}

// Nested classes
class OuterClass { // OuterClass at position (line 107, col 6)
    private int outerField; // outerField at position (line 108, col 16)
    
    class InnerClass { // InnerClass at position (line 110, col 10)
        public void innerMethod() { // innerMethod at position (line 111, col 20)
            System.out.println("Inner method");
        }
    }
    
    static class StaticNestedClass { // StaticNestedClass at position (line 116, col 18)
        public void staticNestedMethod() { // staticNestedMethod at position (line 117, col 20)
            System.out.println("Static nested method");
        }
    }
}

// Generic class
class GenericClass<T> { // GenericClass at position (line 123, col 6)
    private T value; // value at position (line 124, col 14)
    
    public GenericClass(T value) { // GenericClass at position (line 126, col 11)
        this.value = value;
    }
    
    public T getValue() { // getValue at position (line 130, col 13)
        return this.value;
    }
    
    public void setValue(T value) { // setValue at position (line 134, col 16)
        this.value = value;
    }
}
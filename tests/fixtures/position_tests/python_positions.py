# Test fixture for Python tree-sitter position validation
# Line numbers and symbol positions are tested precisely

def simple_function(): # simple_function at position (line 3, col 4)
    pass

def function_with_params(param1, param2): # function_with_params at position (line 6, col 4)
    return param1 + param2

async def async_function(): # async_function at position (line 9, col 10)
    return "async result"

def function_with_types(param1: int, param2: str) -> str: # function_with_types at position (line 12, col 4)
    return str(param1) + param2

class SimpleClass: # SimpleClass at position (line 15, col 6)
    def __init__(self, value): # __init__ at position (line 16, col 8)
        self.value = value
    
    def method(self): # method at position (line 19, col 8)
        return self.value
    
    def method_with_params(self, param1, param2): # method_with_params at position (line 22, col 8)
        return param1 + param2
    
    @staticmethod
    def static_method(): # static_method at position (line 26, col 8)
        return "static"
    
    @classmethod
    def class_method(cls): # class_method at position (line 30, col 8)
        return cls.__name__
    
    @property
    def property_method(self): # property_method at position (line 34, col 8)
        return self.value
    
    @property_method.setter
    def property_method(self, value): # property_method at position (line 38, col 8)
        self.value = value

class InheritedClass(SimpleClass): # InheritedClass at position (line 41, col 6)
    def __init__(self, value, extra): # __init__ at position (line 42, col 8)
        super().__init__(value)
        self.extra = extra
    
    def overridden_method(self): # overridden_method at position (line 46, col 8)
        return f"overridden: {self.value}"

class AbstractBaseClass: # AbstractBaseClass at position (line 49, col 6)
    def abstract_method(self): # abstract_method at position (line 50, col 8)
        raise NotImplementedError

def decorator_function(func): # decorator_function at position (line 53, col 4)
    def wrapper(*args, **kwargs):
        return func(*args, **kwargs)
    return wrapper

@decorator_function
def decorated_function(): # decorated_function at position (line 59, col 4)
    return "decorated"

# Global variables
global_var = 42 # global_var at position (line 63, col 0)
global_string = "hello" # global_string at position (line 64, col 0)

# Lambda functions
lambda_func = lambda x: x * 2 # lambda_func at position (line 67, col 0)

# List comprehension variable
list_comp = [x for x in range(10)] # list_comp at position (line 70, col 0)

# Dictionary comprehension variable
dict_comp = {x: x*2 for x in range(5)} # dict_comp at position (line 73, col 0)

# Import statements (for testing)
# from typing import List, Dict # List, Dict would be at import positions
import os # This creates no symbol positions to test

# Generator function
def generator_function(): # generator_function at position (line 79, col 4)
    yield 1
    yield 2
    yield 3

# Async generator function
async def async_generator(): # async_generator at position (line 84, col 10)
    yield "async1"
    yield "async2"

# Nested functions
def outer_function(): # outer_function at position (line 88, col 4)
    def inner_function(): # inner_function at position (line 89, col 8)
        return "inner"
    return inner_function()

# Exception handling
def function_with_exception(): # function_with_exception at position (line 93, col 4)
    try:
        pass
    except Exception as e: # e would be at exception position
        pass
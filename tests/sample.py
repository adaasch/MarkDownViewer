#!/usr/bin/env python3
"""
A simple Python test file for plain text rendering.
"""

def hello_world():
    """Print hello world."""
    print("Hello, world!")

def fibonacci(n):
    """Generate fibonacci sequence up to n."""
    a, b = 0, 1
    while a < n:
        print(a, end=' ')
        a, b = b, a + b
    print()

if __name__ == "__main__":
    hello_world()
    fibonacci(100)
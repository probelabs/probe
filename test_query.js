function greet(name) {
	return `Hello, ${name}!`;
}

const multiply = (a, b) => a * b;

class Calculator {
	constructor() {
		this.value = 0;
	}

	add(x) {
		this.value += x;
		return this;
	}
}
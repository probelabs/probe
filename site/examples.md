# Code Examples

Here's a sample code block using our custom component:

<CodeBlock
  lang="javascript"
  :code="`const greet = (name) => {
  console.log(\`Hello, \${name}!\`);
};

greet('World');`"
/>

Here's a Python example:

<CodeBlock
  lang="python"
  :code="`def fibonacci(n):
    if n <= 1:
        return n
    else:
        return fibonacci(n-1) + fibonacci(n-2)

# Print first 10 Fibonacci numbers
for i in range(10):
    print(fibonacci(i))`"
/>

Here's a TypeScript example:

<CodeBlock
  lang="typescript"
  :code="`interface User {
  id: number;
  name: string;
  email: string;
}

class UserService {
  private users: User[] = [];

  addUser(user: User): void {
    this.users.push(user);
  }

  findUserById(id: number): User | undefined {
    return this.users.find(user => user.id === id);
  }
}`"
/>

You can also disable the header:

<CodeBlock
  lang="bash"
  :showHeader="false"
  :code="`# Install dependencies
npm install

# Start development server
npm run dev`"
/> 
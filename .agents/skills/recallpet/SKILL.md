```markdown
# recallpet Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches the core development patterns and conventions used in the `recallpet` Rust codebase. It covers file organization, import/export styles, commit message habits, and testing approaches. By following these guidelines, contributors can maintain consistency and quality across the project.

## Coding Conventions

### File Naming
- Use **camelCase** for file names.
  - Example: `petManager.rs`, `userProfile.rs`

### Imports
- Use **relative imports** to reference modules within the project.
  - Example:
    ```rust
    mod petManager;
    use crate::petManager::Pet;
    ```

### Exports
- Use **named exports** for exposing functions, structs, or enums.
  - Example:
    ```rust
    pub struct Pet {
        pub name: String,
        pub age: u8,
    }

    pub fn create_pet(name: &str, age: u8) -> Pet {
        Pet { name: name.to_string(), age }
    }
    ```

### Commit Messages
- Freeform style, sometimes with prefixes.
- Average commit message length: ~55 characters.
  - Example: `add basic pet struct and initialization logic`

## Workflows

### Adding a New Feature
**Trigger:** When you need to implement a new functionality  
**Command:** `/add-feature`

1. Create a new camelCase file for your module (e.g., `featureName.rs`).
2. Implement your feature using relative imports and named exports.
3. Write or update tests in a corresponding `*.test.*` file.
4. Commit your changes with a clear, descriptive message.
5. Push your branch and open a pull request.

### Refactoring Code
**Trigger:** When improving or restructuring existing code  
**Command:** `/refactor`

1. Identify the code to refactor.
2. Make changes while maintaining camelCase file naming and relative imports.
3. Ensure all exports remain named.
4. Update or add tests if necessary.
5. Commit with a message describing the refactor.
6. Push and create a pull request.

### Writing Tests
**Trigger:** When adding or updating tests  
**Command:** `/write-test`

1. Create or update a test file matching the pattern `*.test.*` (e.g., `petManager.test.rs`).
2. Write tests for your module or feature.
3. Run tests using the Rust test runner:
    ```bash
    cargo test
    ```
4. Ensure all tests pass before committing.

## Testing Patterns

- Test files follow the pattern: `*.test.*` (e.g., `petManager.test.rs`).
- The specific testing framework is not specified, but standard Rust testing conventions apply.
- Example test structure:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_create_pet() {
            let pet = create_pet("Buddy", 3);
            assert_eq!(pet.name, "Buddy");
            assert_eq!(pet.age, 3);
        }
    }
    ```

## Commands
| Command        | Purpose                                      |
|----------------|----------------------------------------------|
| /add-feature   | Start the workflow for adding a new feature  |
| /refactor      | Begin a code refactoring workflow            |
| /write-test    | Guide for writing or updating tests          |
```

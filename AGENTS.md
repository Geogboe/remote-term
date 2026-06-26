# Repository Instructions

If the user asks to add something to Obsidian notes, those notes may be in:

- `d:/docs/dev-notes/`

## Git

- Use conventional commits.
- Commit regularly when making meaningful implementation progress.
- Do not delete untracked files you did not create; the user may be working on them.

## Planning And Decisions

- When presenting decisions or open questions, include a recommendation and concise reasoning for that recommendation.
- Keep product specs focused on product behavior and constraints; put agent workflow preferences in this file.

## Development Workflow

When working on real tools, products, or projects, use test and docs driven development:

1. Write or update specs/docs.
2. Write tests and watch them fail.
3. Implement functionality.
4. Confirm tests pass.
5. Run linters/formatting and fix issues.
6. Run smoke, regression, and integration tests.
7. Fully perform manual interactive end-to-end validation when possible.

For simple scripts or narrow one-off tasks, scale this workflow to the risk and scope.

## Engineering Priority

- Prioritize correctness and environment compatibility over implementation simplicity.
- Validate runtime, toolchain, and provider viability before choosing libraries.
- Use language servers or tree-sitter when useful for efficient, accurate edits.

Coding Guidelines
=================

- Use positive conditions in `if` statements for better readability. For example,
  prefer `if collection.is_empty() { None } else { Some(collection) }` over
  checking with negation.

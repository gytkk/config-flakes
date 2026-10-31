"""Shared validation primitives without renderer dependencies."""

class ValidationError(ValueError):
    """Raised when canonical resources cannot be materialized safely."""


def _expect_keys(table: object, expected: set[str], context: str) -> dict:
    if not isinstance(table, dict) or set(table) != expected:
        raise ValidationError(f"{context} has unknown or missing keys")
    return table



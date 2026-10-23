"""A JSON Schema (2020-12) validator for the few keywords the spec schemas use, standard library only.

Any other keyword raises UnsupportedSchema, so a schema never leans on a rule this file would skip.
"""
import json
import re

IGNORED = {"$schema", "title", "description", "$defs"}
SUPPORTED = IGNORED | {"$ref", "type", "properties", "required", "additionalProperties", "items",
                       "minItems", "uniqueItems", "enum", "const", "pattern", "minLength"}
TYPES = {
    "object": lambda v: isinstance(v, dict),
    "array": lambda v: isinstance(v, list),
    "string": lambda v: isinstance(v, str),
    "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
    "number": lambda v: isinstance(v, (int, float)) and not isinstance(v, bool),
    "boolean": lambda v: isinstance(v, bool),
    "null": lambda v: v is None,
}


class UnsupportedSchema(Exception):
    pass


def validate(instance, schema):
    """Every way `instance` breaks `schema`, as `<path>: <reason>` lines; empty when it is valid."""
    _audit(schema, schema, "")
    errors = []
    _check(instance, schema, schema, "", errors)
    return errors


def _key(value):
    return json.dumps(value, sort_keys=True)


def _resolve(ref, root):
    match = re.fullmatch(r"#/\$defs/([^/]+)", ref)
    if not match or match.group(1) not in root.get("$defs", {}):
        raise UnsupportedSchema(f"$ref {ref!r}: only a local #/$defs/<name> that exists")
    return root["$defs"][match.group(1)]


def _audit(schema, root, path):
    """Refuse the whole schema up front, so a keyword under a field an instance lacks still counts."""
    unknown = sorted(set(schema) - SUPPORTED)
    if unknown:
        raise UnsupportedSchema(f"{path or '/'}: unsupported keyword {unknown[0]!r}")
    if "$ref" in schema:
        _resolve(schema["$ref"], root)
    unknown_types = {schema["type"]} if isinstance(schema.get("type"), str) else set(schema.get("type", []))
    if unknown_types - set(TYPES):
        raise UnsupportedSchema(f"{path or '/'}: unknown type {sorted(unknown_types - set(TYPES))[0]!r}")
    children = list(schema.get("properties", {}).items()) + list(schema.get("$defs", {}).items())
    children += [(k, schema[k]) for k in ("items", "additionalProperties") if isinstance(schema.get(k), dict)]
    for name, child in children:
        _audit(child, root, f"{path}/{name}")


def _check(value, schema, root, path, errors):
    where = path or "/"
    if "$ref" in schema:
        _check(value, _resolve(schema["$ref"], root), root, path, errors)
    if "type" in schema:
        allowed = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
        if not any(TYPES[t](value) for t in allowed):
            errors.append(f"{where}: expected {' or '.join(allowed)}")
            return
    if "const" in schema and _key(value) != _key(schema["const"]):
        errors.append(f"{where}: must be {schema['const']!r}")
    if "enum" in schema and _key(value) not in {_key(v) for v in schema["enum"]}:
        errors.append(f"{where}: must be one of {schema['enum']}")
    if isinstance(value, str):
        if len(value) < schema.get("minLength", 0):
            errors.append(f"{where}: shorter than {schema['minLength']}")
        if "pattern" in schema and not re.search(schema["pattern"], value):
            errors.append(f"{where}: does not match {schema['pattern']}")
    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            errors.append(f"{where}: fewer than {schema['minItems']} items")
        if schema.get("uniqueItems") and len({_key(v) for v in value}) != len(value):
            errors.append(f"{where}: items are not unique")
        if "items" in schema:
            for i, item in enumerate(value):
                _check(item, schema["items"], root, f"{path}/{i}", errors)
    if isinstance(value, dict):
        props = schema.get("properties", {})
        for name in schema.get("required", []):
            if name not in value:
                errors.append(f"{where}: missing required {name!r}")
        for name, item in value.items():
            if name in props:
                _check(item, props[name], root, f"{path}/{name}", errors)
            elif schema.get("additionalProperties") is False:
                errors.append(f"{path}/{name}: not allowed")
            elif isinstance(schema.get("additionalProperties"), dict):
                _check(item, schema["additionalProperties"], root, f"{path}/{name}", errors)

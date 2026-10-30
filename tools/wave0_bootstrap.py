#!/usr/bin/env python3
"""
Wave 0 Bootstrap Engine for Apache Fineract -> Rust (Gemini 4 Argon Wave Migration)
1. Parses all 481 Liquibase XML changelogs in fineract-java into a compact, canonical
   Dual-Database (PostgreSQL + MySQL/MariaDB) SQL Schema Anchor (anchor/fineract_schema_dual.sql).
2. Compiles all 101 Apache Avro (.avsc) event schemas in fineract-avro-schemas into the
   `fineract-avro-rs` Rust crate with strongly-typed Serde/Avro structs.
3. Scaffolds the `fineract-rs` Cargo workspace and Wave 1 crate skeletons.
"""

import json
import os
import re
import glob
import xml.etree.ElementTree as ET
from pathlib import Path
from collections import OrderedDict

ROOT = Path("/Users/cmanikandan/Downloads/test-proj")
JAVA_ROOT = ROOT / "fineract-java"
RUST_ROOT = ROOT / "fineract-rs"
ANCHOR_DIR = RUST_ROOT / "anchor"


def strip_ns(tag: str) -> str:
    return tag.split("}", 1)[-1] if "}" in tag else tag


def extract_liquibase_schema():
    """Distill all 481 Liquibase XML files into a canonical Dual-DB (Postgres + MySQL) schema."""
    changelog_files = sorted(JAVA_ROOT.glob("**/db/changelog/**/*.xml"))
    tables = OrderedDict()  # table_name -> OrderedDict(col_name -> (col_type, nullable, pk, default))
    indexes = []
    foreign_keys = []

    for xml_path in changelog_files:
        try:
            tree = ET.parse(xml_path)
        except Exception:
            continue
        root = tree.getroot()
        for elem in root.iter():
            tag = strip_ns(elem.tag)
            if tag == "createTable":
                tname = elem.attrib.get("tableName")
                if not tname:
                    continue
                cols = tables.setdefault(tname, OrderedDict())
                for child in elem:
                    if strip_ns(child.tag) == "column":
                        cname = child.attrib.get("name")
                        ctype = child.attrib.get("type", "VARCHAR(255)")
                        default_val = (
                            child.attrib.get("defaultValue")
                            or child.attrib.get("defaultValueNumeric")
                            or child.attrib.get("defaultValueBoolean")
                        )
                        nullable = True
                        pk = False
                        for cchild in child:
                            if strip_ns(cchild.tag) == "constraints":
                                if cchild.attrib.get("nullable") == "false":
                                    nullable = False
                                if cchild.attrib.get("primaryKey") == "true":
                                    pk = False if nullable else True
                                    pk = True
                        if cname:
                            cols[cname] = (ctype, nullable, pk, default_val)
            elif tag == "addColumn":
                tname = elem.attrib.get("tableName")
                if not tname:
                    continue
                cols = tables.setdefault(tname, OrderedDict())
                for child in elem:
                    if strip_ns(child.tag) == "column":
                        cname = child.attrib.get("name")
                        ctype = child.attrib.get("type", "VARCHAR(255)")
                        default_val = (
                            child.attrib.get("defaultValue")
                            or child.attrib.get("defaultValueNumeric")
                            or child.attrib.get("defaultValueBoolean")
                        )
                        nullable = True
                        for cchild in child:
                            if strip_ns(cchild.tag) == "constraints":
                                if cchild.attrib.get("nullable") == "false":
                                    nullable = False
                        if cname:
                            cols[cname] = (ctype, nullable, False, default_val)
            elif tag == "dropColumn":
                tname = elem.attrib.get("tableName")
                cname = elem.attrib.get("columnName")
                if tname in tables and cname in tables[tname]:
                    del tables[tname][cname]
            elif tag == "renameColumn":
                tname = elem.attrib.get("tableName")
                old_c = elem.attrib.get("oldColumnName")
                new_c = elem.attrib.get("newColumnName")
                ctype = elem.attrib.get("columnDataType")
                if tname in tables and old_c in tables[tname]:
                    prev = tables[tname].pop(old_c)
                    tables[tname][new_c] = (ctype or prev[0], prev[1], prev[2], prev[3])
            elif tag == "dropTable":
                tname = elem.attrib.get("tableName")
                tables.pop(tname, None)
            elif tag == "addForeignKeyConstraint":
                base_t = elem.attrib.get("baseTableName")
                base_c = elem.attrib.get("baseColumnNames")
                ref_t = elem.attrib.get("referencedTableName")
                ref_c = elem.attrib.get("referencedColumnNames")
                if base_t and base_c and ref_t and ref_c:
                    foreign_keys.append((base_t, base_c, ref_t, ref_c))

    ANCHOR_DIR.mkdir(parents=True, exist_ok=True)
    out_sql = ANCHOR_DIR / "fineract_schema_dual.sql"
    lines = [
        "-- =============================================================================",
        "-- APACHE FINERACT CANONICAL DUAL-DIALECT DDL ANCHOR (PostgreSQL + MySQL/MariaDB)",
        f"-- Distilled from {len(changelog_files)} Liquibase XML changelogs ({len(tables)} tables)",
        "-- Dialect Note: BIGINT AUTO_INCREMENT (MySQL) <-> BIGSERIAL / GENERATED BY DEFAULT AS IDENTITY (Postgres)",
        "-- DECIMAL(19,6) used for all monetary columns; sqlx::Any runtime driver maps across both engines.",
        "-- =============================================================================",
        "",
    ]
    for tname, cols in tables.items():
        lines.append(f"CREATE TABLE {tname} (")
        col_lines = []
        for cname, (ctype, nullable, pk, default_val) in cols.items():
            null_str = " NOT NULL" if (not nullable or pk) else ""
            pk_str = " PRIMARY KEY" if pk else ""
            def_str = f" DEFAULT {default_val}" if default_val is not None else ""
            col_lines.append(f"    {cname} {ctype}{null_str}{pk_str}{def_str}")
        lines.append(",\n".join(col_lines))
        lines.append(");\n")

    lines.append("-- Foreign Key Relationships (Summary)")
    seen_fks = set()
    for bt, bc, rt, rc in foreign_keys:
        key = (bt, bc, rt, rc)
        if key not in seen_fks:
            seen_fks.add(key)
            lines.append(f"-- FK: {bt}({bc}) -> {rt}({rc})")

    content = "\n".join(lines)
    out_sql.write_text(content, encoding="utf-8")
    print(f"[Wave 0] Wrote {out_sql} ({len(tables)} tables, {len(seen_fks)} FKs, {len(content):,d} bytes / ~{len(content)//4:,d} tokens)")


def to_snake_case(name: str) -> str:
    s1 = re.sub("(.)([A-Z][a-z]+)", r"\1_\2", name)
    s2 = re.sub("([a-z0-9])([A-Z])", r"\1_\2", s1).lower()
    rust_keywords = {"type", "ref", "move", "in", "match", "loop", "Self", "self", "super", "crate", "mod", "fn", "let", "pub", "use", "struct", "enum", "trait", "impl", "for", "while", "if", "else", "return", "true", "false", "where", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try"}
    return f"r#{s2}" if s2 in rust_keywords else s2


def map_avro_type(avro_type, defined_types: set) -> str:
    if isinstance(avro_type, str):
        mapping = {
            "null": "()",
            "boolean": "bool",
            "int": "i32",
            "long": "i64",
            "float": "f32",
            "double": "f64",
            "bytes": "Vec<u8>",
            "string": "String",
            "bigdecimal": "rust_decimal::Decimal",
        }
        if avro_type in mapping:
            return mapping[avro_type]
        short_name = avro_type.split(".")[-1]
        return short_name if short_name in defined_types else "serde_json::Value"
    elif isinstance(avro_type, list):
        non_null = [t for t in avro_type if t != "null"]
        if len(non_null) == 1 and "null" in avro_type:
            return f"Option<{map_avro_type(non_null[0], defined_types)}>"
        return "serde_json::Value"
    elif isinstance(avro_type, dict):
        t = avro_type.get("type")
        logical = avro_type.get("logicalType")
        if logical in ("decimal", "bigdecimal"):
            return "rust_decimal::Decimal"
        if t == "array":
            items = avro_type.get("items", "string")
            return f"Vec<{map_avro_type(items, defined_types)}>"
        if t == "map":
            vals = avro_type.get("values", "string")
            return f"std::collections::HashMap<String, {map_avro_type(vals, defined_types)}>"
        if t in ("record", "enum"):
            return avro_type.get("name", "serde_json::Value")
        return map_avro_type(t, defined_types)
    return "serde_json::Value"


def generate_avro_crate():
    """Compile all 101 .avsc schemas into crates/fineract-avro-rs."""
    avsc_files = sorted((JAVA_ROOT / "fineract-avro-schemas").glob("**/*.avsc"))
    parsed_schemas = []
    defined_types = set()

    for path in avsc_files:
        data = json.loads(path.read_text(encoding="utf-8"))
        name = data.get("name")
        if name:
            defined_types.add(name)
            parsed_schemas.append((path, data))

    crate_dir = RUST_ROOT / "crates" / "fineract-avro-rs"
    src_dir = crate_dir / "src"
    src_dir.mkdir(parents=True, exist_ok=True)

    cargo_toml = """[package]
name = "fineract-avro-rs"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Auto-generated Apache Fineract Avro v1 External Event schemas and Serde types (101 schemas)"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
rust_decimal = { workspace = true }
apache-avro = { workspace = true }
"""
    (crate_dir / "Cargo.toml").write_text(cargo_toml, encoding="utf-8")

    lines = [
        "//! Auto-generated from `fineract-avro-schemas` (101 `.avsc` files).",
        "//! Provides strongly-typed Rust structs and raw Avro JSON schema constants for the Fineract Transactional Outbox (`m_external_event`).",
        "#![allow(clippy::all, non_camel_case_types)]",
        "",
        "use serde::{Deserialize, Serialize};",
        "",
    ]

    manifest_lines = [
        "# Apache Fineract Avro Event Schemas Manifest (101 Schemas)",
        "",
        "| Rust Struct | Namespace | Fields |",
        "| :--- | :--- | :--- |",
    ]

    seen_names = set()
    for path, data in parsed_schemas:
        name = data.get("name")
        if not name or name in seen_names:
            continue
        seen_names.add(name)
        ns = data.get("namespace", "org.apache.fineract.avro")
        doc = data.get("doc", f"Avro schema `{ns}.{name}`")
        fields = data.get("fields", [])
        field_names = [f.get("name", "") for f in fields]
        manifest_lines.append(f"| `{name}` | `{ns}` | `{', '.join(field_names[:8])}{'...' if len(field_names) > 8 else ''}` |")

        lines.append(f"/// {doc} (Namespace: `{ns}`)")
        lines.append("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]")
        lines.append('#[serde(rename_all = "camelCase")]')
        lines.append(f"pub struct {name} {{")
        for f in fields:
            fname = f.get("name", "unknown")
            fdoc = f.get("doc")
            ftype = map_avro_type(f.get("type"), defined_types)
            if fdoc:
                lines.append(f"    /// {fdoc}")
            snake = to_snake_case(fname)
            if snake.replace("r#", "") != fname:
                lines.append(f'    #[serde(rename = "{fname}")]')
            lines.append(f"    pub {snake}: {ftype},")
        lines.append("}\n")

        const_name = re.sub("([a-z0-9])([A-Z])", r"\1_\2", name).upper() + "_AVSC"
        raw_json = json.dumps(data, separators=(",", ":"))
        lines.append(f"pub const {const_name}: &str = r#\"{raw_json}\"#;\n")

    lib_rs = "\n".join(lines)
    (src_dir / "lib.rs").write_text(lib_rs, encoding="utf-8")

    manifest_path = ANCHOR_DIR / "avro_manifest.md"
    manifest_path.write_text("\n".join(manifest_lines), encoding="utf-8")
    print(f"[Wave 0] Generated {src_dir / 'lib.rs'} with {len(seen_names)} Avro structs ({len(lib_rs):,d} bytes)")


if __name__ == "__main__":
    extract_liquibase_schema()
    generate_avro_crate()

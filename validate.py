#!/usr/bin/env python3
"""Validate records in CSV, JSON, or JSON Lines datasets."""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import math
import re
import sys
from pathlib import Path
from typing import Any


DATA_ERROR_EXIT = 2
VALIDATION_ERROR_EXIT = 1
MISSING_VALUES = {"", "null", "nan"}
TYPE_NAMES = {"string", "integer", "number", "boolean", "date"}


class DataError(Exception):
    """Raised when the configuration or input cannot be read."""


def load_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise DataError(f"无法读取JSON文件 {path}: {error}") from error


def normalize_config(config: Any) -> dict[str, dict[str, Any]]:
    if not isinstance(config, dict):
        raise DataError("配置格式非法：顶层必须是JSON对象")
    if "fields" not in config:
        raise DataError("配置错误：缺少fields")
    fields = config["fields"]
    if not isinstance(fields, dict) or not fields:
        raise DataError("配置错误：fields必须是非空对象")

    normalized: dict[str, dict[str, Any]] = {}
    for name, definition in fields.items():
        if not isinstance(name, str) or not name:
            raise DataError("配置错误：字段名必须是非空字符串")
        if not isinstance(definition, dict):
            raise DataError(f"配置错误：字段{name}的定义必须是对象")
        field_type = definition.get("type", "string")
        if field_type not in TYPE_NAMES:
            raise DataError(f"配置错误：字段{name}的类型不支持: {field_type}")
        normalized[name] = {
            **definition,
            "type": field_type,
            "required": definition.get("required", True),
        }
        for rule_name in ("min_length", "max_length"):
            if rule_name in definition and not isinstance(definition[rule_name], (int, float)):
                raise DataError(f"配置错误：字段{name}的{rule_name}必须是数值")
        if field_type in {"integer", "number"}:
            for rule_name in ("min", "max"):
                if rule_name in definition and not isinstance(definition[rule_name], (int, float)):
                    raise DataError(f"配置错误：字段{name}的{rule_name}必须是数值")
        elif field_type == "date":
            for rule_name in ("min", "max"):
                if rule_name in definition:
                    try:
                        dt.date.fromisoformat(str(definition[rule_name]))
                    except (TypeError, ValueError) as error:
                        raise DataError(f"配置错误：字段{name}的日期范围非法") from error
        if "regex" in definition and not isinstance(definition["regex"], str):
            raise DataError(f"配置错误：字段{name}的regex必须是字符串")
        try:
            if "regex" in definition:
                re.compile(definition["regex"])
        except re.error as error:
            raise DataError(f"配置错误：字段{name}的正则非法: {error}") from error
        if "allowed_characters" in definition and not isinstance(definition["allowed_characters"], str):
            raise DataError(f"配置错误：字段{name}的allowed_characters必须是字符串")
    return normalized


def load_records(path: Path) -> list[dict[str, Any]]:
    suffix = path.suffix.lower()
    try:
        if suffix == ".csv":
            with path.open("r", encoding="utf-8-sig", newline="") as handle:
                reader = csv.DictReader(handle)
                if reader.fieldnames is None:
                    raise DataError("输入错误：CSV没有表头")
                return list(reader)
        if suffix == ".jsonl":
            records = []
            with path.open("r", encoding="utf-8") as handle:
                for line_number, line in enumerate(handle, 1):
                    if not line.strip():
                        continue
                    try:
                        records.append(json.loads(line))
                    except json.JSONDecodeError as error:
                        raise DataError(f"输入错误：第{line_number}行不是合法JSON: {error}") from error
            return records
        if suffix == ".json":
            value = load_json(path)
            if not isinstance(value, list):
                raise DataError("输入错误：JSON顶层必须是数组")
            return value
    except OSError as error:
        raise DataError(f"无法读取输入文件 {path}: {error}") from error
    raise DataError(f"输入错误：不支持的文件类型 {suffix or '(无后缀)'}")


def is_missing(value: Any) -> bool:
    if value is None or (isinstance(value, float) and math.isnan(value)):
        return True
    return isinstance(value, str) and value.strip().lower() in MISSING_VALUES


def to_number(value: Any, field: str) -> tuple[float | None, str | None]:
    if isinstance(value, bool):
        return None, f"字段{field}格式非法"
    if isinstance(value, (int, float)):
        if isinstance(value, float) and not math.isfinite(value):
            return None, f"字段{field}格式非法"
        return float(value), None
    if isinstance(value, str):
        try:
            number = float(value)
        except ValueError:
            return None, f"字段{field}格式非法"
        if math.isfinite(number):
            return number, None
    return None, f"字段{field}格式非法"


def parse_bool(value: Any, field: str) -> tuple[bool | None, str | None]:
    if isinstance(value, bool):
        return value, None
    if isinstance(value, str):
        lowered = value.strip().lower()
        if lowered in {"true", "1"}:
            return True, None
        if lowered in {"false", "0"}:
            return False, None
    return None, f"字段{field}格式非法"


def parse_date(value: Any, field: str) -> tuple[dt.date | None, str | None]:
    if not isinstance(value, str):
        return None, f"字段{field}格式非法"
    try:
        return dt.date.fromisoformat(value), None
    except ValueError:
        return None, f"字段{field}格式非法"


def validate_record(record: Any, row_number: int, fields: dict[str, dict[str, Any]]) -> list[str]:
    if not isinstance(record, dict):
        return [f"第{row_number}条记录格式非法：必须是对象"]

    errors: list[str] = []
    for field, definition in fields.items():
        if is_missing(record.get(field)):
            if definition["required"]:
                errors.append(f"第{row_number}条：字段{field}为空或缺失")
            continue

        value = record[field]
        field_type = definition["type"]
        if field_type == "integer":
            if isinstance(value, bool) or not isinstance(value, int):
                if not isinstance(value, str) or re.fullmatch(r"[+-]?\d+", value.strip()) is None:
                    errors.append(f"第{row_number}条：字段{field}格式非法")
                    continue
            normalized: Any = int(value) if isinstance(value, str) else value
        elif field_type == "number":
            normalized, error = to_number(value, field)
            if error:
                errors.append(f"第{row_number}条：{error}")
                continue
        elif field_type == "boolean":
            normalized, error = parse_bool(value, field)
            if error:
                errors.append(f"第{row_number}条：{error}")
                continue
        elif field_type == "date":
            normalized, error = parse_date(value, field)
            if error:
                errors.append(f"第{row_number}条：{error}")
                continue
        else:
            if not isinstance(value, str):
                errors.append(f"第{row_number}条：字段{field}格式非法")
                continue
            normalized = value

        if "regex" in definition:
            candidate = normalized.isoformat() if field_type == "date" else str(normalized)
            if re.fullmatch(definition["regex"], candidate) is None:
                errors.append(f"第{row_number}条：字段{field}格式非法")
        allowed = definition.get("allowed_characters")
        if allowed is not None:
            text = value if isinstance(value, str) else str(normalized)
            if any(character not in allowed for character in text):
                errors.append(f"第{row_number}条：字段{field}格式非法")

        minimum = definition.get("min")
        maximum = definition.get("max")
        if field_type in {"integer", "number"}:
            if minimum is not None and float(normalized) < float(minimum):
                errors.append(f"第{row_number}条：字段{field}低于下限{minimum}")
            if maximum is not None and float(normalized) > float(maximum):
                errors.append(f"第{row_number}条：字段{field}超过上限{maximum}")
        elif field_type == "date":
            try:
                minimum_date = dt.date.fromisoformat(str(minimum)) if minimum is not None else None
                maximum_date = dt.date.fromisoformat(str(maximum)) if maximum is not None else None
            except ValueError as error:
                raise DataError(f"配置错误：字段{field}的日期范围非法") from error
            if minimum_date and normalized < minimum_date:
                errors.append(f"第{row_number}条：字段{field}早于下限{minimum_date.isoformat()}")
            if maximum_date and normalized > maximum_date:
                errors.append(f"第{row_number}条：字段{field}晚于上限{maximum_date.isoformat()}")

        min_length = definition.get("min_length")
        max_length = definition.get("max_length")
        if min_length is not None or max_length is not None:
            length = len(value) if isinstance(value, str) else len(str(normalized))
            if min_length is not None and length < min_length:
                errors.append(f"第{row_number}条：字段{field}长度低于下限{min_length}")
            if max_length is not None and length > max_length:
                errors.append(f"第{row_number}条：字段{field}长度超过上限{max_length}")
    return errors


def write_failures(path: Path, errors: list[str], error_count: int) -> None:
    try:
        with path.open("w", encoding="utf-8") as handle:
            json.dump({"error_count": error_count, "errors": errors}, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
    except OSError as error:
        raise DataError(f"无法写入失败明细 {path}: {error}") from error


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="待验证数据集（.json/.jsonl/.csv）")
    parser.add_argument("--config", "-c", type=Path, required=True, help="字段定义JSON")
    parser.add_argument("--output", "-o", type=Path, default=Path("validation_errors.json"), help="失败明细JSON")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        fields = normalize_config(load_json(args.config))
        records = load_records(args.input)
        errors = [
            error
            for row_number, record in enumerate(records, 1)
            for error in validate_record(record, row_number, fields)
        ]
        write_failures(args.output, errors, len(errors))
    except DataError as error:
        print(f"错误：{error}", file=sys.stderr)
        return DATA_ERROR_EXIT

    for error in errors:
        print(error)
    print(f"校验完成：共{len(records)}条记录，失败明细{len(errors)}项")
    return VALIDATION_ERROR_EXIT if errors else 0


if __name__ == "__main__":
    sys.exit(main())

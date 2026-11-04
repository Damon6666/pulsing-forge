import json
import subprocess
import sys
from pathlib import Path


def run_validator(tmp_path, config, records, extension="jsonl"):
    config_path = tmp_path / "config.json"
    input_path = tmp_path / f"input.{extension}"
    output_path = tmp_path / "errors.json"
    config_path.write_text(json.dumps(config), encoding="utf-8")
    if extension == "jsonl":
        input_path.write_text("".join(json.dumps(record) + "\n" for record in records), encoding="utf-8")
    elif extension == "json":
        input_path.write_text(json.dumps(records), encoding="utf-8")
    else:
        input_path.write_text(records, encoding="utf-8")
    result = subprocess.run(
        [sys.executable, "validate.py", str(input_path), "-c", str(config_path), "-o", str(output_path)],
        capture_output=True,
        text=True,
    )
    details = json.loads(output_path.read_text(encoding="utf-8")) if output_path.exists() else None
    return result, details


def test_accumulates_errors_and_writes_details(tmp_path):
    config = {
        "fields": {
            "name": {"type": "string", "allowed_characters": "abc ", "max_length": 3},
            "age": {"type": "integer", "min": 1, "max": 120},
        }
    }
    records = [{"name": "abcd!", "age": 0}, {"name": "ab", "age": 121}, {"name": "", "age": "x"}]
    result, details = run_validator(tmp_path, config, records)
    assert result.returncode == 1
    assert "字段name为空或缺失" in result.stdout
    assert "字段age低于下限1" in result.stdout
    assert "字段age超过上限120" in result.stdout
    assert "字段name长度超过上限3" in result.stdout
    assert details["error_count"] == len(details["errors"]) == 6


def test_valid_json_and_jsonl(tmp_path):
    config = {"fields": {"id": {"type": "string", "regex": "[A-Z]{2}\\d{2}"}}}
    for extension in ("json", "jsonl"):
        result, details = run_validator(tmp_path, config, [{"id": "AB12"}], extension)
        assert result.returncode == 0
        assert details == {"error_count": 0, "errors": []}


def test_allowed_characters_rejects_disallowed_character(tmp_path):
    config = {"fields": {"name": {"type": "string", "allowed_characters": "abc "}}}
    result, details = run_validator(tmp_path, config, [{"name": "ab!"}])
    assert result.returncode == 1
    assert "字段name格式非法" in result.stdout
    assert details["errors"] == ["第1条：字段name格式非法"]


def test_csv_and_date_range(tmp_path):
    config = {"fields": {"day": {"type": "date", "min": "2024-01-01", "max": "2024-12-31"}}}
    result, details = run_validator(tmp_path, config, "day\n2023-12-31\n2024-06-01\n", "csv")
    assert result.returncode == 1
    assert "字段day早于下限2024-01-01" in result.stdout
    assert len(details["errors"]) == 1


def test_invalid_input_returns_independent_code(tmp_path):
    result, _ = run_validator(tmp_path, {"fields": {"id": {}}}, [], "json")
    assert result.returncode == 0
    input_path = tmp_path / "input.jsonl"
    input_path.write_text("{broken\n", encoding="utf-8")
    config_path = tmp_path / "config.json"
    output_path = tmp_path / "errors.json"
    broken = subprocess.run(
        [sys.executable, "validate.py", str(input_path), "-c", str(config_path), "-o", str(output_path)],
        capture_output=True,
        text=True,
    )
    assert broken.returncode == 2
    assert "输入错误" in broken.stderr


def test_config_errors_fail_immediately(tmp_path):
    result, _ = run_validator(tmp_path, {"fields": {"id": {"type": "unknown"}}}, [])
    assert result.returncode == 2
    assert "配置错误" in result.stderr

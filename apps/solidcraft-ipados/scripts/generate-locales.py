#!/usr/bin/env python3
"""Extract native chrome messages from the real catalogues, before/after pending fold."""

import json
import pathlib
import sys


def extract(repository: pathlib.Path) -> dict[str, dict[str, str]]:
    directory = repository / "assets" / "locales"
    catalogues = {
        source.stem: json.loads(source.read_text(encoding="utf-8"))
        for source in sorted(directory.glob("*.json"))
    }
    for source in sorted((directory / "pending").glob("*.json")):
        for locale, messages in json.loads(source.read_text(encoding="utf-8")).items():
            catalogues.setdefault(locale, {}).update(messages)
    return {
        locale: {key: value for key, value in messages.items() if key.startswith("ui.ipados.")}
        for locale, messages in catalogues.items()
        if any(key.startswith("ui.ipados.") for key in messages)
    }


if __name__ == "__main__":
    repository = pathlib.Path(sys.argv[1]).resolve()
    destination = pathlib.Path(sys.argv[2])
    catalogues = extract(repository)
    required = {"ui.ipados.foundation_error", "ui.ipados.foundation_loading", "ui.ipados.foundation_ready"}
    if not required.issubset(catalogues.get("en", {})):
        raise SystemExit("native English catalogue is incomplete")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(catalogues, ensure_ascii=False, sort_keys=True), encoding="utf-8")

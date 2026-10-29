from __future__ import annotations

from .errors import CliError


def prompt_yes_no(question: str, *, default: bool = True) -> bool:
    suffix = " [Y/n] " if default else " [y/N] "
    while True:
        try:
            answer = input(question + suffix).strip().lower()
        except EOFError as exc:
            raise CliError("Input closed; use --yes for non-interactive runs.") from exc
        if not answer:
            return default
        if answer in ("y", "yes"):
            return True
        if answer in ("n", "no"):
            return False
        print("Please answer y or n.")


def prompt_optional(question: str) -> str:
    try:
        return input(question).strip()
    except EOFError as exc:
        raise CliError("Input closed; pass --name explicitly.") from exc

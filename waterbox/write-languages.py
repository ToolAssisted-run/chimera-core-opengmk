#!/usr/bin/env python3
"""Writes languages/<code>.json for every language, from i18n/texts.py.

English (languages/en.json) is the table in that file; every other language's
list is in the same order. Fails if a language has too few or too many texts.

usage: write-languages.py
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "i18n"))
from texts import EN, KEYS, TRANSLATIONS  # noqa: E402


def write(code, table):
    path = os.path.join(HERE, "languages", code + ".json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"texts": dict(sorted(table.items()))}, f, ensure_ascii=False, indent="\t")
        f.write("\n")


def main():
    os.makedirs(os.path.join(HERE, "languages"), exist_ok=True)
    write("en", EN)
    for code, texts in TRANSLATIONS.items():
        if len(texts) != len(KEYS):
            sys.exit("%s has %d texts, the table has %d" % (code, len(texts), len(KEYS)))
        for key, text in zip(KEYS, texts):
            # the same %1 and %2 as English, so a text never loses an argument
            for mark in ("%1", "%2"):
                if (mark in EN[key]) != (mark in text):
                    sys.exit("%s %s: %s differs from English" % (code, key, mark))
        write(code, dict(zip(KEYS, texts)))
    print("wrote %d languages" % (len(TRANSLATIONS) + 1))


if __name__ == "__main__":
    main()

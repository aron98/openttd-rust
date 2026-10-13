"""Defined diagnostics exercise fmt precision and each VM failure boundary."""

from .case import Case


def diagnostic_cases() -> list[Case]:
    keys = {
        "ascii49": '"' + "a" * 49 + '"',
        "ascii50": '"' + "a" * 50 + '"',
        "ascii51": '"' + "a" * 51 + '"',
        "unicode-byte49": '"' + "a" * 49 + '€z"',
        "unicode50": '"' + "€" * 50 + 'z"',
        "nul": '"a\\0b"',
        "nul-boundary": '"' + "a" * 49 + '\\0z"',
        "boolean": "true",
        "float14": "1.234567",
        "float-small": "0.00001234567",
        "float-negative-zero": "-0.0",
    }
    result = [
        Case(
            f"diagnostic-{name}-{operation}",
            f"local t={{a=1}};\n{statement}",
            "runtime_error",
        )
        for name, key in keys.items()
        for operation, statement in (
            ("get", f"return t[{key}];"),
            ("set", f"t[{key}]=7;"),
            ("delete", f"return delete t[{key}];"),
            ("compound", f"t[{key}]+=7;"),
        )
    ]
    result.extend(
        Case(
            f"diagnostic-capture-ascii{length}",
            f"local f=function():({'a' * length}) {{return 1;}};\nreturn 0;",
            "runtime_error",
        )
        for length in (49, 50, 51, 60)
    )
    result.extend(
        (
            Case("diagnostic-null-concat", 'return ""+null;'),
            Case(
                "diagnostic-clone-closure",
                "local f=function(){return 1;};\nreturn clone f;",
                "runtime_error",
            ),
            Case("diagnostic-newslot-scalar", "local x=7;\nx.a<-1;", "runtime_error"),
            Case("diagnostic-newslot-array", "local x=[];\nx.a<-1;", "runtime_error"),
        )
    )
    return result

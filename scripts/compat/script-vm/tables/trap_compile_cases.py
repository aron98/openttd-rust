"""Malformed exception statements reach pinned compiler diagnostic boundaries."""

from .case import Case


def trap_compile_cases() -> list[Case]:
    return [
        Case(f"trap-compiler-{name}", source, "compile_error", error_trace=True)
        for name, source in {
            "missing-catch": "try {}",
            "catch-open": "try {} catch {}",
            "catch-empty": "try {} catch(){}",
            "catch-numeric-name": "try {} catch(1){}",
            "catch-close": "try {} catch(e {}",
            "catch-body": "try {} catch(e)",
            "try-body": "try",
            "catch-without-try": "catch(e){}",
            "throw-semicolon": "throw;",
            "throw-end": "throw",
            "throw-comma": "throw 1,;",
            "reserved-local": "local try=1;",
            "reserved-function": "function try(){}",
            "catch-break": "try {} catch(e){break;}",
            "catch-continue": "try {} catch(e){continue;}",
            "protected-semicolon": "try throw 1; catch(e){}",
        }.items()
    ]

"""Source-defined cleanup, error history and arbitrary-object boundary probes."""

from .case import Case
from .closure_cases import joined_source


def trap_boundary_cases() -> list[Case]:
    sources = {
        "comma-throw": "local n=0;\ntry{throw (n=3,n+1);}catch(e){return n*10+e;}",
        "rethrow-null": joined_source(
            "try{try{throw null;}catch(e){throw e;}}",
            "catch(e){return e==null;}",
        ),
        "array-typecheck": 'try{return array("bad");}catch(e){return e;}',
        "table-compare": "try{return {}<1;}catch(e){return e;}",
        "array-compare": "try{return []<true;}catch(e){return e;}",
        "closure-compare": "try{return function(){}<assert;}catch(e){return e;}",
        "float-compare": 'try{return 1.234567<"other";}catch(e){return e;}',
        "single-throw-statement": "try throw 7 catch(e){return e;}",
        "history-survives-normal-try": joined_source(
            'try{throw "old";}catch(e){}',
            "try{local x=1;}catch(e){return 99;}return 1;",
        ),
        "ignored-error-protected": joined_source(
            "const C=3;\nlocal f=function(){this=7;\n",
            "try{return this.C+=1;}catch(e){return 99;}};\nreturn f();",
        ),
        "catch-default-failure": joined_source(
            "try{try{throw 3;}catch(e){local f=function(x=absent){return x;};\n",
            "return f();}}catch(outer){return outer;}",
        ),
        "catch-abandoned-local": joined_source(
            "local a=[null];\ntry{local t={};\na[0]=t;\nthrow 1;}",
            "catch(e){}return a[0]==null;",
        ),
        "nested-switch-continue": joined_source(
            "local n=0;\nfor(local i=0;i<3;i++){try{switch(i){",
            "case 1:continue;\ndefault:n+=i;\nbreak;}}catch(e){return 99;}}",
            "return n;",
        ),
        "nested-break-leaves-outer": joined_source(
            "try{while(true){try{break;}catch(inner){return 99;}}",
            "throw 7;}catch(outer){return outer;}",
        ),
    }
    result = [
        Case(f"trap-boundary-{name}", source, error_trace=True)
        for name, source in sources.items()
    ]
    for length in (49, 50, 51, 60):
        key = "é" * length + "\\0Z"
        result.append(
            Case(
                f"trap-boundary-compare-{length}",
                f'try{{return "{key}"<true;}}catch(e){{return e;}}',
                error_trace=True,
            )
        )
    result.extend(
        [
            Case(
                "trap-boundary-uncaught-nul",
                'throw "a\\0Z";',
                "runtime_error",
                error_trace=True,
            ),
            Case(
                "trap-boundary-catch-local-expires",
                "try{throw 1;}catch(e){}return e;",
                "runtime_error",
                error_trace=True,
            ),
        ]
    )
    return result

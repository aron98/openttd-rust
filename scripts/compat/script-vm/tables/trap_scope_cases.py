"""Frozen defined native trap discovery inputs with passive error-object traces."""

from .case import Case
from .closure_cases import joined_source


def trap_scope_cases() -> list[Case]:
    return [
        Case(
            "trap-caller_throw",
            joined_source(
                "local f=function(){throw 17;\n};\n try { f();\n } catch(e) ",
                "{ return e;\n }",
            ),
            error_trace=True,
        ),
        Case(
            "trap-caller_native_failure",
            joined_source(
                "local f=function(){assert(false);\n};\n try { f();\n } catc",
                "h(e) { return e;\n }",
            ),
            error_trace=True,
        ),
        Case(
            "trap-catch_shadow",
            joined_source(
                "local e=11;\n local x=0;\n try { throw 7;\n } catch(e) { x=",
                "e;\n } return e*10+x;\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-catch_capture",
            joined_source(
                "local f=null;\n try { throw 9;\n } catch(e) { f=function()",
                ":(e){return e;\n};\n } return f();\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-catch_default",
            joined_source(
                "try { throw 12;\n } catch(e) { local f=function(x=e){retu",
                "rn x;\n};\n return f();\n }",
            ),
            error_trace=True,
        ),
        Case(
            "trap-rethrow_identity",
            joined_source(
                "local a=[7];\n try { try { throw a;\n } catch(e) { e[0]=8;",
                "\n throw e;\n } } catch(e) { return e==a && a[0]==8;\n }",
            ),
            error_trace=True,
        ),
        Case(
            "trap-array_identity",
            joined_source(
                "local a=[7];\n try { throw a;\n } catch(e) { e[0]=9;\n retu",
                "rn e==a && a[0]==9;\n }",
            ),
            error_trace=True,
        ),
        Case(
            "trap-table_identity",
            joined_source(
                "local a={x=7};\n try { throw a;\n } catch(e) { e.x=9;\n ret",
                "urn e==a && a.x==9;\n }",
            ),
            error_trace=True,
        ),
        Case(
            "trap-closure_identity",
            joined_source(
                "local f=function(){return 7;\n};\n try { throw f;\n } catch",
                "(e) { return e==f && e()==7;\n }",
            ),
            error_trace=True,
        ),
        Case(
            "trap-native_closure_identity",
            "try { throw assert;\n } catch(e) { return e==assert;\n }",
            error_trace=True,
        ),
        Case(
            "trap-caught_null",
            "try { throw null;\n } catch(e) { return e==null;\n }",
            error_trace=True,
        ),
        Case(
            "trap-caught_bool",
            "try { throw true;\n } catch(e) { return e;\n }",
            error_trace=True,
        ),
        Case(
            "trap-caught_float",
            "try { throw 1.25;\n } catch(e) { return e;\n }",
            error_trace=True,
        ),
        Case(
            "trap-caught_nul",
            'try { throw "left\\x00right";\n } catch(e) { return e;\n }',
            error_trace=True,
        ),
        Case(
            "trap-caught_unicode",
            joined_source(
                'try { /* Árvíz */ throw "héő";\n } catch(e) { return e;\n ',
                "}",
            ),
            error_trace=True,
        ),
        Case(
            "trap-variadic_unwind",
            joined_source(
                "local f=function(...){throw vargv[0];\n};\ntry{f(7,8,9);\n}",
                "catch(e){return e;\n}",
            ),
            error_trace=True,
        ),
        Case(
            "trap-variadic_nested",
            joined_source(
                "local g=function(...){throw vargv[1];\n};\nlocal f=functio",
                "n(...) :(g){local x=g(3,4);\nreturn x;\n};\ntry{f(1,2);\n}ca",
                "tch(e){return e;\n}",
            ),
            error_trace=True,
        ),
    ]

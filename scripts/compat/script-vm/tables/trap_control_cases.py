"""Frozen defined native trap discovery inputs with passive error-object traces."""

from .case import Case
from .closure_cases import joined_source


def trap_control_cases() -> list[Case]:
    return [
        Case(
            "trap-normal_pop",
            "try { local x=2;\n } catch(e) { return 99;\n } return 7;\n",
            error_trace=True,
        ),
        Case(
            "trap-nested",
            joined_source(
                "try { try { throw 7;\n } catch(inner) { throw inner+2;\n }",
                " } catch(outer) { return outer;\n }",
            ),
            error_trace=True,
        ),
        Case(
            "trap-return_inside_try",
            joined_source(
                "local f=function(){try { return 5;\n } catch(e) {return 9",
                "9;\n}};\n return f();\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-return_call_inside_try",
            joined_source(
                "local g=function(){throw 5;\n};\n local f=function():(g){t",
                "ry { return g();\n } catch(e){return e+1;\n}};\n return f()",
                ";\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-return_null_inside_try",
            joined_source(
                "local f=function(){try { return;\n } catch(e){return 99;\n",
                "}};\nreturn f();\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-caller_catches_tail",
            joined_source(
                "local g=function(){throw 6;\n};\nlocal f=function():(g){re",
                "turn g();\n};\ntry{f();\n}catch(e){return e;\n}",
            ),
            error_trace=True,
        ),
        Case(
            "trap-continue_inside_try",
            joined_source(
                "local n=0;\nfor(local i=0;\ni<4;\ni++){try{if(i==1)continue",
                ";\nn+=i;\n}catch(e){return 99;\n}}return n;\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-break_inside_try",
            joined_source(
                "local n=0;\nwhile(true){try{n++;\nif(n==2)break;\n}catch(e)",
                "{return 99;\n}}return n;\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-loop_inside_try",
            joined_source(
                "try{local n=0;\nfor(local i=0;\ni<5;\ni++){if(i==1)continue",
                ";\nif(i==3)break;\nn+=i;\n}throw n;\n}catch(e){return e;\n}",
            ),
            error_trace=True,
        ),
        Case(
            "trap-switch_inside_try",
            joined_source(
                "try{switch(1){case 1:break;\n}throw 7;\n}catch(e){return e",
                ";\n}",
            ),
            error_trace=True,
        ),
        Case(
            "trap-for_catch_continue",
            joined_source(
                "local n=0;\nfor(local i=0;\ni<3;\ni++){try{throw i;\n}catch(",
                "e){n+=e;\ncontinue;\n}}return n;\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-foreach_throw",
            joined_source(
                "local n=0;\nforeach(x in [1,2,3]){try{throw x;\n}catch(e){",
                "n+=e;\n}}return n;\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-do_throw",
            joined_source(
                "local n=0;\ndo{try{throw n;\n}catch(e){n=e+1;\n}}while(n<3)",
                ";\nreturn n;\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-print_effects",
            joined_source(
                'print("before");\ntry{print("try");\nthrow 5;\n}catch(e){pr',
                'int("catch");\n}print("after");\nreturn 1;\n',
            ),
            error_trace=True,
        ),
    ]

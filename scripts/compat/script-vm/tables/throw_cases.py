"""Frozen defined native trap discovery inputs with passive error-object traces."""

from .case import Case
from .closure_cases import joined_source


def throw_cases() -> list[Case]:
    return [
        Case(
            "trap-root_effects",
            joined_source(
                "::mark<-1;\ntry{::mark=2;\nthrow 7;\n}catch(e){::mark+=e;\n}",
                "return ::mark;\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-caught_divide",
            "try{return 7/0;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_modulo",
            "try{return 7%0;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_arithmetic_type",
            "try{return null-1;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_bitwise_type",
            "try{return true&1;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_negate_type",
            "try{local x=null;\nreturn -x;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_bitnot_type",
            "try{return ~true;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_comparison_type",
            "try{return true<1;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_missing_index",
            "try{return {}.missing;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_bad_call",
            "try{local f=null;\nf();\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_arity",
            joined_source(
                "local f=function(x){return x;\n};\ntry{f();\n}catch(e){retu",
                "rn e;\n}",
            ),
            error_trace=True,
        ),
        Case(
            "trap-caught_array_negative",
            "try{return array(-1);\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_capture_error",
            joined_source(
                "try{local f=function():(missing){return missing;\n};\n}cat",
                "ch(e){return e;\n}",
            ),
            error_trace=True,
        ),
        Case(
            "trap-caught_clone_error",
            "try{return clone 1;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_delete_error",
            "try{return delete 1[0];\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_foreach_error",
            "try{foreach(x in 1){}}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_update_error",
            "try{local x=null;\nx++;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_compound_error",
            "try{local x={a=null};\nx.a+=1;\n}catch(e){return e;\n}",
            error_trace=True,
        ),
        Case(
            "trap-caught_owner",
            "try{throw [7];\n}catch(e){}return 1;\n",
            error_trace=True,
        ),
        Case(
            "trap-caught_cycle_owner",
            joined_source(
                "local a=[null];\na[0]=a;\ntry{throw a;\n}catch(e){}a=null;\n",
                "return 1;\n",
            ),
            error_trace=True,
        ),
        Case(
            "trap-uncaught_null",
            "throw null;\n",
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_bool",
            "throw true;\n",
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_integer",
            "throw 42;\n",
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_float",
            "throw 1.25;\n",
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_string",
            'throw "hello";\n',
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_nul",
            'throw "a\\x00b";\n',
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_array",
            "throw [7];\n",
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_table",
            "throw {x=7};\n",
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_closure",
            "throw function(){return 3;\n};\n",
            "runtime_error",
            error_trace=True,
        ),
        Case(
            "trap-uncaught_native",
            "throw assert;\n",
            "runtime_error",
            error_trace=True,
        ),
    ]

"""Pinned-source call witnesses; the earlier corpus remains unchanged."""

from .case import Case


def cases() -> list[Case]:
    return [
        Case("type-integer", "return type(7);", "return"),
        Case("type-native", "return type(print);", "return"),
        Case("assert-success", "return assert(true);", "return"),
        Case("assert-false", "return assert(false);", "runtime_error"),
        Case("assert-null", "return assert(null);", "runtime_error"),
        Case("assert-arity-zero", "return assert();", "runtime_error"),
        Case("assert-arity-many", "return assert(1,2);", "runtime_error"),
        Case("type-arity", "return type(1,2);", "runtime_error"),
        Case(
            "print-scalars",
            'print("A\\n");\nprint(7);\nprint(true);\nprint(null);\nreturn 3;',
            "return",
        ),
        Case("print-nul", 'print("a\\0b\\n");\nreturn 1;', "return"),
        Case(
            "print-order",
            'print("A\\n");\nlocal t=type(print("B\\n"));\nprint(t);\nreturn t;',
            "return",
        ),
        Case("print-arity", 'return print("A", "B");', "runtime_error"),
        Case("array-default", "local a=array(3);\nreturn a[2];", "return"),
        Case("array-fill", "local a=array(3,7);\nreturn a[0]+a[2];", "return"),
        Case("array-float", "local a=array(2.9,7);\nreturn a[1];", "return"),
        Case("array-negative", "return array(-1);", "runtime_error"),
        Case("array-wrong-type", 'return array("2");', "runtime_error"),
        Case("array-bool", "return array(true);", "runtime_error"),
        Case("array-arity", "return array();", "runtime_error"),
        Case(
            "array-extra",
            "local n=0;\nlocal a=array(2,7,n++);\nreturn a[0]+n;",
            "return",
        ),
        Case(
            "array-alias",
            "local t={key=1};\nlocal a=array(2,t);\na[0].key=9;\nreturn a[1].key;",
            "return",
        ),
        Case(
            "array-cycle",
            "local t={};\nlocal a=array(2,t);\nt.a<-a;\nreturn a[1].a==a;",
            "return",
        ),
        Case("local-callee-mutation", "local f=assert;\nreturn f(f=type);", "return"),
        Case(
            "member-callee-mutation",
            "local t={f=assert};\nreturn t.f(t.f=type);",
            "return",
        ),
        Case(
            "nested-side-effects",
            "local n=0;\nlocal a=array(n++,type(n++));\nreturn n;",
            "return",
        ),
        Case(
            "late-error",
            """held<-{key=7};
print("before\\n");
held.key=9;
assert(false);
return 1;""",
            "runtime_error",
        ),
        Case("after-this", "local f=type;\nthis=null;\nreturn f(this);", "return"),
        Case("after-this-root-call", "this=7;\nreturn type(this);", "return"),
        Case("noncallable", "local f=7;\nreturn f(1);", "runtime_error"),
        Case("print-real-nul", 'print("a\\0b\\n");\nreturn 1;', "return"),
        Case(
            "print-record-spoof",
            'print("return 999 integer 1\\nruntime_error 0\\n");\nassert(false);',
            "runtime_error",
        ),
        Case("print-empty", 'print("");\nreturn 1;', "return"),
        Case("chain-return-call", "return array(1,type)[0](7);", "return"),
        Case(
            "callee-replaced-root", "local f=type;\ntype=7;\nreturn f(type);", "return"
        ),
        Case(
            "callee-replaced-member-args",
            "local t={f=type};\nreturn t.f(delete t.f);",
            "return",
        ),
        Case(
            "callee-computed-key", 'local t={f=type};\nreturn t["f"](false);', "return"
        ),
        Case(
            "bad-call-eval-effect",
            "local n=0;\nlocal f=null;\nreturn f(n++);",
            "runtime_error",
        ),
        Case("array-negative-float", "return array(-1.9);", "runtime_error"),
        Case("array-negative-fraction", "return array(-0.9);", "return"),
        Case("array-zero", "return array(0);", "return"),
        Case("array-null-mask", "return array(null);", "runtime_error"),
        Case("array-function-mask", "return array(type);", "runtime_error"),
        Case("array-array-mask", "return array([]);", "runtime_error"),
        Case("array-table-mask", "return array({});", "runtime_error"),
        Case("type-this-argument-mutation", "return type(this=7);", "return"),
        Case(
            "call-array-element-mutation",
            "local a=[assert];\nreturn a[0](a[0]=type);",
            "return",
        ),
        Case(
            "calls-in-foreach",
            'local out="";\nforeach(v in [1,true,null]) out+=type(v);\nreturn out;',
            "return",
        ),
        Case(
            "discard-many", "for(local n=0;n<20;n++) array(2,{});\nreturn 1;", "return"
        ),
        Case(
            "late-arity-print",
            'return type(print("before"),print("after"));',
            "runtime_error",
        ),
        Case(
            "late-mask-print",
            'return array(print("before"),print("after"));',
            "runtime_error",
        ),
        Case("literal-callee", "return (7)(1);", "runtime_error"),
        Case("call-trailing-comma", "return type(7,);", "compile_error"),
    ]

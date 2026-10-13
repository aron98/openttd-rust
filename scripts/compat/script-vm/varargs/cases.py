# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Pinned-original varargs discovery scenarios; run through compare.py."""

from typing import Final

from .case import Case
from .controls import CONTROLS

CASES: Final = (
    Case(
        "nested-default-alias",
        (
            "local t={x=1};\nlocal f=function(...) { local g=function(a=va"
            "rgv[0]) { return a.x; };\nvargv[0].x=9;\nreturn g; };\nlocal g="
            "f(t);\nreturn g();"
        ),
        "return",
        b"integer 9",
    ),
    Case(
        "fixed-vargv",
        "local f=function(a) { return vargv[0]; };\nreturn f(7);",
        "runtime_error",
        b"the function doesn't have var args",
    ),
    Case(
        "trailing-comma",
        "local f=function(a,...) { return vargc; };\nreturn f(1,2,);",
        "compile_error",
        b"expression expected, found ')'",
    ),
    Case(
        "discovery",
        "local f=function(...) { return vargc; };\nreturn f(1,2,3);",
        "return",
        b"integer 3",
    ),
    Case(
        "minimum",
        "local f=function(a,b,...) { return a+b+vargc; };\nreturn f(2,3);",
        "return",
        b"integer 5",
    ),
    Case(
        "fixed-extra",
        (
            "local f=function(a,...) { return a+vargc*10+vargv[0]+vargv[1"
            "]; };\nreturn f(1,2,3);"
        ),
        "return",
        b"integer 26",
    ),
    Case(
        "float-index",
        "local f=function(...) { return vargv[1.9]+vargv[-0.5]; };\nreturn f(2,7);",
        "return",
        b"integer 9",
    ),
    Case(
        "aggregate-alias",
        (
            "local t={x=1};\nlocal f=function(...) { vargv[0].x=9;\nreturn "
            "vargv[0]==vargv[1]; };\nreturn f(t,t) && t.x==9;"
        ),
        "return",
        b"bool 1",
    ),
    Case(
        "nested",
        (
            "local f=function(...) { local g=function(...) { return vargc"
            "+vargv[0]; };\nlocal n=g(7,8);\nreturn n+vargc+vargv[0]; };\nre"
            "turn f(3);"
        ),
        "return",
        b"integer 13",
    ),
    Case(
        "tail",
        (
            "function f(n,...) { if(n==0) return vargv[0];\nreturn f(n-1,v"
            "argv[0]+1); }\nreturn f(12,3);"
        ),
        "return",
        b"integer 15",
    ),
    Case(
        "nested-default",
        (
            "local f=function(...) { local g=function(a=vargv[0]) { retur"
            "n a; };\nreturn g(); };\nreturn f(9);"
        ),
        "return",
        b"integer 9",
    ),
    Case(
        "escaped-alias",
        (
            "local t={x=1};\nlocal f=function(...) { local a=vargv[0];\nret"
            "urn function():(a) { return a.x; }; };\nlocal g=f(t);\nt.x=9;\n"
            "return g();"
        ),
        "return",
        b"integer 9",
    ),
    Case(
        "fixed-vargc",
        "local f=function(a=7) { return vargc+a; };\nreturn f();",
        "return",
        b"integer 7",
    ),
    Case("main-vargc", "return vargc;", "return", b"integer 0"),
    Case(
        "raw-print",
        (
            "local f=function(...) { print(vargv[0]);\nreturn vargc; };\nre"
            'turn f("a\\0b");'
        ),
        "return",
        b"integer 1",
    ),
    Case(
        "too-few",
        "local f=function(a,b,...) { return 0; };\nreturn f(1);",
        "runtime_error",
        b"wrong number of parameters",
    ),
    Case(
        "empty-index",
        'local f=function(...) { return vargv["x"]; };\nreturn f();',
        "runtime_error",
        b"the function doesn't have var args",
    ),
    Case(
        "bad-index",
        'local f=function(...) { return vargv["x"]; };\nreturn f(3);',
        "runtime_error",
        b"indexing 'vargv' with string",
    ),
    Case(
        "bool-index",
        "local f=function(...) { return vargv[true]; };\nreturn f(3);",
        "runtime_error",
        b"indexing 'vargv' with bool",
    ),
    Case(
        "negative-index",
        "local f=function(...) { return vargv[-1]; };\nreturn f(3);",
        "runtime_error",
        b"vargv index out of range",
    ),
    Case(
        "upper-index",
        "local f=function(...) { return vargv[1]; };\nreturn f(3);",
        "runtime_error",
        b"vargv index out of range",
    ),
    Case(
        "nested-fixed-index",
        (
            "local f=function(...) { local g=function() { return vargv[0]"
            "; };\nreturn g(); };\nreturn f(7);"
        ),
        "runtime_error",
        b"the function doesn't have var args",
    ),
    Case(
        "late-failure",
        (
            "local t={x=1};\nlocal f=function(...) { vargv[0].x=9;\nprint(v"
            "argv[0].x);\nreturn vargv[1]; };\nreturn f(t);"
        ),
        "runtime_error",
        b"vargv index out of range",
    ),
    Case(
        "defaults-forbidden",
        "local f=function(a=1,...) { return a; };",
        "compile_error",
        b"function with default parameters cannot have variable number of parameters",
    ),
    Case(
        "ellipsis-last",
        "local f=function(...,a) { return a; };",
        "compile_error",
        b"expected ')'",
    ),
    Case(
        "bare-vargv",
        "local f=function(...) { return vargv; };",
        "compile_error",
        b"expected '['",
    ),
    Case(
        "store-vargv",
        "local f=function(...) { vargv[0]=9; };",
        "compile_error",
        b"can't assign expression",
    ),
    *CONTROLS,
)

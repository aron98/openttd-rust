# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through: python3 -m scripts.compat.script-vm.varargs.compare
"""Defined boundary controls; unsafe original tail failures are Rust-only tests."""

from pathlib import Path
from typing import Final

from .case import Case

CONTROLS: Final = (
    Case(
        "deep-base-wrap",
        (Path(__file__).parent / "fixtures/deep.nut").read_text(),
        "return",
        b"integer 329",
        (100000,),
    ),
    Case(
        "fixed-too-many",
        "local f=function(a){return a;};\nreturn f(1,2);",
        "runtime_error",
        b"wrong number of parameters",
    ),
    Case(
        "fixed-too-few",
        "local f=function(a){return a;};\nreturn f();",
        "runtime_error",
        b"wrong number of parameters",
    ),
    Case(
        "empty-tail-arity",
        (
            "local bad=function(a){return a;};\nlocal f=function(...) :(ba"
            "d) {return bad();};\nreturn f();"
        ),
        "runtime_error",
        b"wrong number of parameters",
    ),
    Case(
        "normal-active-arity",
        (
            "local bad=function(a){return a;};\nlocal f=function(...) :(ba"
            "d) {print(vargc);\nlocal x=bad();\nreturn x;};\nreturn f(1);"
        ),
        "runtime_error",
        b"wrong number of parameters",
    ),
    Case(
        "tail-default",
        (
            "local g=function(a=7){return a+vargc;};\nlocal f=function(..."
            ") :(g) {return g();};\nreturn f(1,2,3);"
        ),
        "return",
        b"integer 7",
    ),
    Case(
        "fixed-tail-variadic",
        (
            "local g=function(...){return vargc+vargv[0]+vargv[1];};\nloca"
            "l f=function(a) :(g) {return g(a,8);};\nreturn f(3);"
        ),
        "return",
        b"integer 13",
    ),
    Case(
        "tail-count-growth",
        (
            "function f(n,...) {if(n==0) return vargc;\nreturn f(n-1,1,2,3"
            ");}\nreturn f(2,9);"
        ),
        "return",
        b"integer 3",
    ),
    Case(
        "tail-restores-parent",
        (
            "local g=function(...){return vargc;};\nlocal f=function(...) "
            ":(g) {local child=function(...) :(g) {return g(1,2,3);};\nloc"
            "al n=child(7);\nreturn n+vargv[0]+vargc;};\nreturn f(8,9);"
        ),
        "return",
        b"integer 13",
    ),
    Case(
        "native-tail-error",
        "local f=function(...){print(vargc);\nreturn assert(false);};\nreturn f(1);",
        "runtime_error",
        b"assertion failed",
    ),
    Case(
        "noncallable-tail",
        "local f=function(...){return vargv[0]();};\nreturn f(1);",
        "runtime_error",
        b"attempt to call 'integer'",
    ),
    Case(
        "successful-tail-late-error",
        (
            "local g=function(...){return vargv[1];};\nlocal f=function(.."
            ".) :(g) {return g(vargv[0]);};\nreturn f(1);"
        ),
        "runtime_error",
        b"vargv index out of range",
    ),
    Case(
        "array-alias",
        (
            "local a=[1];\nlocal f=function(...){vargv[0][0]=9;\nreturn var"
            "gv[0]==vargv[1];};\nreturn f(a,a) && a[0]==9;"
        ),
        "return",
        b"bool 1",
    ),
    Case(
        "closure-alias",
        (
            "local g=function(a){return a+1;};\nlocal f=function(...){retu"
            "rn vargv[0](8);};\nreturn f(g);"
        ),
        "return",
        b"integer 9",
    ),
    Case(
        "index-evaluation",
        (
            "local i=0;\nlocal f=function(...) :(i) {return vargv[i]+vargc"
            ";};\nreturn f(7,8);"
        ),
        "return",
        b"integer 9",
    ),
    Case(
        "empty-index-side-effect",
        (
            "local f=function(...){local i=function(){print(9);\nreturn tr"
            "ue;};\nreturn vargv[i()];};\nreturn f();"
        ),
        "runtime_error",
        b"the function doesn't have var args",
    ),
    Case(
        "main-index",
        "return vargv[0];",
        "runtime_error",
        b"the function doesn't have var args",
    ),
    Case(
        "local-index-store",
        "local f=function(...){local i=0;\nvargv[i]=9;};",
        "compile_error",
        b"can't assign expression",
    ),
    Case(
        "count-store",
        "local f=function(...){vargc=9;};",
        "compile_error",
        b"can't assign expression",
    ),
    Case("two-dots", "local f=function(..){};", "compile_error", b"invalid token '..'"),
    Case(
        "keyword-parameter",
        "local f=function(vargc){};",
        "compile_error",
        b"expected 'IDENTIFIER'",
    ),
)

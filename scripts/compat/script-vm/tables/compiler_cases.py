# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through python3 -m scripts.compat.script-vm.tables.compare NATIVE RUST OUTPUT
"""Defined compiler failures and Unicode/lazy-input controls from pinned throw sites."""

from .case import Case


def compiler_cases() -> list[Case]:
    return [
        Case("compiler-table-colon", "return {a:1};", "compile_error"),
        Case("compiler-table-string-key", 'return {"a"=1};', "compile_error"),
        Case("compiler-call-trailing-comma", "print(1,);", "compile_error"),
        Case(
            "compiler-default-before-required",
            "local f=function(a=1,b) {return a;};",
            "compile_error",
        ),
        Case(
            "compiler-default-varargs",
            "local f=function(a=1,...) {return a;};",
            "compile_error",
        ),
        Case(
            "compiler-ellipsis-last",
            "local f=function(...,a) {return a;};",
            "compile_error",
        ),
        Case(
            "compiler-bare-vargv",
            "local f=function(...) {return vargv;};",
            "compile_error",
        ),
        Case(
            "compiler-store-vargv",
            "local f=function(...) {vargv[0]=9;};",
            "compile_error",
        ),
        Case("compiler-delete-local", "local a=1;\nreturn delete a;", "compile_error"),
        Case("compiler-delete-expression", "return delete (1+2);", "compile_error"),
        Case("compiler-create-local", "local a=1;\na<-2;", "compile_error"),
        Case("compiler-assign-expression", "1=2;", "compile_error"),
        Case("compiler-break", "break;", "compile_error"),
        Case("compiler-continue", "continue;", "compile_error"),
        Case("compiler-missing-expression", "return );", "compile_error"),
        Case("compiler-missing-identifier", "local =1;", "compile_error"),
        Case("compiler-missing-delimiter", "return (1;", "compile_error"),
        Case("compiler-semicolon", "local a=1 local b=2;", "compile_error"),
        Case("compiler-unterminated-comment", "/* x", "compile_error"),
        Case("compiler-unterminated-string", 'return "x', "compile_error"),
        Case("compiler-newline-string", 'return "x\ny";', "compile_error"),
        Case("compiler-escape", 'return "\\q";', "compile_error"),
        Case("compiler-hex-escape", 'return "\\xZ";', "compile_error"),
        Case("compiler-empty-character", "return '';", "compile_error"),
        Case("compiler-long-character", "return 'ab';", "compile_error"),
        Case("compiler-octal", "return 08;", "return"),
        Case("compiler-exponent", "return 1e+;", "compile_error"),
        Case("compiler-two-dots", "return ..;", "compile_error"),
        Case("compiler-verbatim", "return @1;", "compile_error"),
        Case("compiler-unicode-string", 'return "€ő漢";', "return"),
        Case("compiler-unicode-comments", "/* €ő漢 */\n// €\nreturn 3;", "return"),
        Case(
            "compiler-unicode-before-error", 'local s="€";\nreturn );', "compile_error"
        ),
        Case("compiler-early-error-unreached-unicode", "return ); €", "compile_error"),
        Case("compiler-rhs-before-local-create", "local a=1;\na<-);", "compile_error"),
        Case(
            "compiler-rhs-before-capture-store",
            "local x=1;\nlocal f=function():(x){x=);};",
            "compile_error",
        ),
        Case("compiler-lvalue-before-rhs", "1=);", "compile_error"),
        Case(
            "compiler-parameter-separator", "local f=function(a b) {};", "compile_error"
        ),
        Case(
            "compiler-capture-separator",
            "local f=function():(a b) {};",
            "compile_error",
        ),
        Case(
            "compiler-enum-member", "enum E {a=1};\nreturn E.missing;", "compile_error"
        ),
        Case("compiler-constant-scalar", "const x=true;", "compile_error"),
        Case("compiler-constant-negative-string", 'const x=-"s";', "compile_error"),
        Case("compiler-invalid-octal", "return 018;", "compile_error"),
        Case("compiler-hex-width", "return 0x11111111111111111;", "compile_error"),
        Case("compiler-ascii-control", "return \x01;", "compile_error"),
        Case("compiler-newline-index", "local a=[1];\nreturn a\n[0];", "compile_error"),
        Case("compiler-foreach-name", "foreach(1 in []) {}", "compile_error"),
        Case("compiler-foreach-in", "foreach(a of []) {}", "compile_error"),
        Case("compiler-do-while", "do {} if(1);", "compile_error"),
        Case("compiler-explicit-root-name", "return ::1;", "compile_error"),
        Case("compiler-empty-escape", 'return "\\', "compile_error"),
        Case("compiler-nul-termination", "return 7;\x00€", "return"),
    ]

"""Original-valid fixed-arity closures and strict failure witnesses."""

from .case import Case


def joined_source(*parts: str) -> str:
    """Join literal pieces without changing original source bytes."""
    return "".join(parts)


def cases() -> list[Case]:
    return [
        Case("named-add", "function add(a,b) { return a+b; }\nreturn add(2,3);"),
        Case("anonymous", "local f=function(a) { return a+1; };\nreturn f(7);"),
        Case(
            "explicit-copy",
            "local x=3;\nlocal f=function():(x) { return x; };\nx=7;\nreturn f();",
        ),
        Case(
            "no-auto-capture",
            "local x=3;\nlocal f=function() { return x; };\nreturn f();",
            "runtime_error",
        ),
        Case(
            "capture-aggregate",
            joined_source(
                "local t={x=1};\nlocal f=function():(t) { return t.x; };\nt",
                ".x=7;\nreturn f();",
            ),
        ),
        Case(
            "capture-transitive",
            joined_source(
                "local x=4;\nlocal f=function():(x) { return function():(x",
                ") { return x; }; };\nreturn f()();",
            ),
        ),
        Case(
            "capture-symbol",
            "x<-8;\nlocal f=function():(x) { return x; };\nx=9;\nreturn f();",
        ),
        Case(
            "capture-increment-copy",
            "local x=1;\nlocal f=function():(x) { return ++x; };\nreturn f()*10+f();",
        ),
        Case(
            "default-creation",
            "local x=1;\nlocal f=function(a=x++) { return a; };\nreturn f()*10+x;",
        ),
        Case(
            "default-alias",
            "local f=function(a={x=1}) { a.x+=1;\nreturn a.x; };\nreturn f()*10+f();",
        ),
        Case(
            "arity", "local f=function(a) { return a; };\nreturn f();", "runtime_error"
        ),
        Case(
            "member-this",
            "local t={x=7,f=function() { return this.x; }};\nreturn t.f();",
        ),
        Case("nested-native", "local f=function(a) { return type(a); };\nreturn f(7);"),
        Case(
            "recursive-tail",
            "function f(n) { if(n==0) return 0;\nreturn f(n-1); }\nreturn f(20);",
        ),
        Case(
            "closure-cycle",
            joined_source(
                "local t={};\nlocal f=function():(t) { return t.f; };\nt.f<",
                "-f;\nreturn f()==f;",
            ),
        ),
        Case(
            "non-tail-recursion",
            joined_source(
                "function sum(n) { if(n==0) return 0;\nreturn n+sum(n-1); ",
                "}\nreturn sum(20);",
            ),
        ),
        Case(
            "closure-in-array",
            "local x=7;\nlocal a=[function():(x){return x;}];\nreturn a[0]();",
        ),
        Case(
            "closure-in-table",
            joined_source(
                "local t={n=5};\nlocal f=function():(t){return t.n;};\nloca",
                "l box={f=f};\nreturn box.f();",
            ),
        ),
        Case("closure-type", "return typeof function(){return 1;};"),
        Case(
            "closure-distinct",
            joined_source(
                "local make=function(){return function(){return 1;};};\nre",
                "turn make()!=make();",
            ),
        ),
        Case(
            "default-function",
            "local f=function(g=function(){return 9;}){return g();};\nreturn f();",
        ),
        Case(
            "default-two",
            joined_source(
                "local n=1;\nlocal f=function(a=n++,b=n++){return a*10+b;}",
                ";\nreturn f()*100+f(4)*10+n;",
            ),
        ),
        Case(
            "default-table-identity",
            "local f=function(a={}){return a;};\nreturn f()==f();",
        ),
        Case(
            "explicit-default-override",
            "local f=function(a=3,b=4){return a*10+b;};\nreturn f(8,9);",
        ),
        Case(
            "captured-native",
            "local f=function():(type){return type(7);};\nreturn f();",
        ),
        Case(
            "captured-mutation",
            joined_source(
                "local t={x=1};\nlocal f=function():(t){t.x=8;\nreturn t;};",
                "\nf();\nreturn t.x;",
            ),
        ),
        Case(
            "captured-compound-copy",
            "local x=1;\nlocal f=function():(x){x+=2;\nreturn x;};\nreturn f()*10+f();",
        ),
        Case(
            "argument-order",
            joined_source(
                "local t={x=0};\nlocal a=function():(t){t.x+=1;\nreturn t.x",
                ";};\nlocal f=function(x,y){return x*10+y;};\nreturn f(a(),",
                "a());",
            ),
        ),
        Case(
            "nested-print-nul",
            'local f=function(x){print(x);\nreturn type(x);};\nreturn f("a\\0b\\n");',
        ),
        Case(
            "nested-native-array",
            "local f=function(n){local a=array(n,7);\nreturn a[1];};\nreturn f(2);",
        ),
        Case(
            "nested-this-reassigned",
            "x<-7;\nlocal f=function(){this=1;\nreturn x;};\nreturn f();",
        ),
        Case(
            "body-assert-error",
            joined_source(
                'local f=function(){print("before");\nassert(false);\nretur',
                "n 7;};\nreturn f();",
            ),
            "runtime_error",
        ),
        Case(
            "nested-arity-error",
            joined_source(
                "local g=function(a){return a;};\nlocal f=function():(g){r",
                "eturn g();};\nreturn f();",
            ),
            "runtime_error",
        ),
        Case(
            "non-callable-error",
            "local f=function(a){return a();};\nreturn f(7);",
            "runtime_error",
        ),
        Case(
            "failed-capture-error",
            "local held={x=1};\nreturn function():(held,missing){return held;};",
            "runtime_error",
        ),
        Case(
            "default-error-before-closure",
            "local f=function(a=assert(false)){return a;};\nreturn f();",
            "runtime_error",
        ),
        Case(
            "excess-arity-error",
            "local f=function(a){return a;};\nreturn f(1,2);",
            "runtime_error",
        ),
        Case(
            "free-assignment-error",
            "local x=1;\nlocal f=function():(x){x=2;};\nreturn f();",
            "compile_error",
        ),
        Case(
            "conditional-tail-barrier",
            joined_source(
                "local a=function(){return 1;};\nlocal b=function(){return",
                " 2;};\nlocal f=function(x):(a,b){return x?a():b();};\nretu",
                "rn f(false)+f(true);",
            ),
        ),
        Case(
            "logical-tail-barrier",
            joined_source(
                "local a=function(){return 1;};\nlocal b=function(){return",
                " 2;};\nlocal f=function():(a,b){return a()&&b();};\nreturn",
                " f();",
            ),
        ),
        Case("compound-constant-store-false", "const C=3;\nreturn this.C+=1;"),
        Case("compound-scalar-store-false", "const C=3;\nthis=7;\nreturn this.C+=1;"),
        Case(
            "compound-root-fallback-store", "x<-3;\nthis={};\nthis.x+=1;\nreturn ::x;"
        ),
        Case(
            "nested-native-array-fill-alias",
            joined_source(
                "local t={};\nlocal f=function():(t){return array(2,t);};\n",
                "local a=f();\na[0].x<-7;\nreturn a[1].x;",
            ),
        ),
        Case("returned-closure-type", "return function(){return 7;};"),
    ]

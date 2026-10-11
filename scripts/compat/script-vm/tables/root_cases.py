"""Native default-root and register-zero consumer sources."""

# Explicit source joins preserve exact fixture bytes under the strict type checker.
# ruff: noqa: ISC003
from .case import Case


def root_cases() -> list[Case]:
    return [
        Case(
            "native-root-builtin",
            "return _intsize_;\n",
            "return",
        ),
        Case(
            "native-root-clone",
            'local copied = clone this;\nlocal result = "";\nfore'
            + "ach (key, value in copied) result = result + key +"
            + ' ";";\nreturn result;\n',
            "return",
        ),
        Case(
            "native-root-closure-alias",
            "local copied = clone this;\nreturn copied.print == " + "print;\n",
            "return",
        ),
        Case(
            "native-root-cycle",
            "self <- this;\nlocal copied = clone this;\nreturn co"
            + "pied.self == this;\n",
            "return",
        ),
        Case(
            "native-root-integer",
            "this[7] <- 42;\nreturn this[7];\n",
            "return",
        ),
        Case(
            "native-root-keys",
            'local result = "";\nforeach (key, value in this) re'
            + 'sult = result + key + ";";\nreturn result;\n',
            "return",
        ),
        Case(
            "native-this-array",
            "this=[7];\nreturn this[0];",
            "return",
        ),
        Case(
            "native-this-delete-own",
            "x<-7;\nthis={};\nreturn delete this.x;",
            "runtime_error",
        ),
        Case(
            "native-this-get-fallback",
            "x<-7;\nthis={};\nreturn x;",
            "return",
        ),
        Case(
            "native-this-native-clone-error",
            "return clone print;",
            "runtime_error",
        ),
        Case(
            "native-this-null",
            "this=null;\nreturn this;",
            "return",
        ),
        Case(
            "native-this-print-type",
            "return typeof print;",
            "return",
        ),
        Case(
            "native-this-set-fallback",
            "x<-7;\nthis={};\nx=9;\nreturn ::x;",
            "return",
        ),
        Case(
            "native-this-table",
            "this={};\nthis.x<-7;\nreturn this.x;",
            "return",
        ),
        Case(
            "native-root-return",
            "return this;",
            "return",
        ),
        Case(
            "native-root-clone-return",
            "return clone this;",
            "return",
        ),
        Case(
            "native-closure-return",
            "return print;",
            "return",
        ),
        Case(
            "native-closure-truth",
            "return !print;",
            "return",
        ),
        Case(
            "native-closure-distinct",
            "return print!=assert;",
            "return",
        ),
        Case(
            "native-closure-array-alias",
            "local a=[print];\nreturn a[0]==print;",
            "return",
        ),
        Case(
            "native-root-clone-default-values",
            "local c=clone this;\nreturn c._version_+c._charsize"
            + "_+c._intsize_+c._floatsize_;",
            "return",
        ),
        Case(
            "native-root-live-delete",
            "local n=0;\nforeach(k,v in this){delete this[k];\nn+" + "+;}\nreturn n;",
            "return",
        ),
        Case(
            "native-root-closure-delete",
            "local p=delete this.print;\nreturn typeof p;",
            "return",
        ),
        Case(
            "native-root-owned-self",
            "this.self<-this;\nlocal c=clone this;\nreturn c.self" + "==this;",
            "return",
        ),
        Case(
            "native-root-foreach-values",
            'local n=0;\nforeach(k,v in this){if(typeof v=="func'
            + 'tion")n++;}\nreturn n;',
            "return",
        ),
        Case(
            "native-root-this-fallback-alias",
            "x<-7;\nlocal original=this;\nthis={};\nlocal alias=th"
            + "is;\nalias.x=9;\nreturn original.x;",
            "return",
        ),
        Case(
            "native-root-delete-no-fallback",
            "x<-7;\nthis={};\nreturn delete x;",
            "runtime_error",
        ),
        Case("native-this-array-global", "x<-7;\nthis=[];\nreturn x;"),
        Case("native-this-bool-global", "x<-7;\nthis=true;\nreturn x;"),
        Case("native-this-closure-global", "x<-7;\nthis=print;\nreturn x;"),
        Case("native-this-float-global", "x<-7;\nthis=1.0;\nreturn x;"),
        Case("native-this-integer-global", "x<-7;\nthis=7;\nreturn x;"),
        Case("native-this-null-global", "x<-7;\nthis=null;\nreturn x;"),
        Case("native-this-string-global", 'x<-7;\nthis="text";\nreturn x;'),
        Case(
            "native-array-no-fallback",
            "this[1]<-7;\nthis=[];\nreturn this[1];",
            "runtime_error",
        ),
        Case(
            "native-scalar-set-no-fallback",
            "x<-7;\nthis=null;\nx=9;\nreturn ::x;",
            "runtime_error",
        ),
        Case(
            "native-mixed-type-no-fallback",
            "x<-7;\nthis=1;\nlocal alias=1.0;\nreturn alias.x;",
            "runtime_error",
        ),
        Case("native-prefix-this", "this=7;\n++this;\nreturn this;", "return"),
        Case(
            "native-postfix-this",
            "this=7;\nlocal old=this++;\nreturn old*10+this;",
            "return",
        ),
        Case("native-compound-this", "this=7;\nthis+=3;\nreturn this;", "return"),
    ]

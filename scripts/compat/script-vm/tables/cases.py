"""Real source inputs; outcomes are required, values/order come only from native."""

# Keep explicit source-fragment joins required by the strict Python type checker.
# ruff: noqa: ISC003
from .case import Case, literal
from .objects import object_cases


def cases() -> list[Case]:
    result = [
        Case("empty", "return typeof {};"),
        Case("return-table", "return {a=1};"),
        Case("dot", "local t={a=1};\nreturn t.a;"),
        Case("bracket", 'local t={a=1};\nreturn t["a"];'),
        Case("replace", "local t={a=1};\nt.a=2; return t.a;"),
        Case("newslot", "local t={};\nt.a<-2; return t.a;"),
        Case("replace-newslot", "local t={a=1};\nt.a<-2; return t.a;"),
        Case("duplicate", "local t={a=1,a=2,a=3,a=4,a=5};\nreturn t.a;"),
        Case("optional-comma", "local t={a=1 b=2 c=3,};\nreturn t.a+t.b+t.c;"),
        Case("computed", "local x=1; local t={[x+1]=9};\nreturn t[2];"),
        Case("comma-key", "local t={[(1,2)]=7};\nreturn t[2];"),
        Case("alias", "local t={a=1};\nlocal q=t; q.a=9; return t.a;"),
        Case("identity", "local t={};\nlocal q=t; return t==q;"),
        Case("distinct", "local t={};\nlocal q={};\nreturn t==q;"),
        Case("truth", "return !{};"),
        Case("root", "slot<-{a=1};\nslot.a=4; return ::slot.a;"),
        Case("nested-block", "local n=0; {local t={a=3};\nn=t.a;}\nreturn n;"),
        Case(
            "key-types",
            "local t={[1]=2,[true]=3,[1.0]=5,[false]=7,[-1]=11};\n"
            + "return t[1]*100+t[true]*10+t[1.0];",
        ),
        Case("zeros", "local t={[0.0]=3,[-0.0]=7};\nreturn t[0.0]*10+t[-0.0];"),
        Case("null-get", "local t={};\nreturn t[null];", "runtime_error"),
        Case("null-set", "local t={};\nt[null]=7; return 9;"),
        Case(
            "null-set-full", "local t={[0]=2};\nt[null]=7; return 9;", "runtime_error"
        ),
        Case("null-new", "local t={};\nt[null]<-7; return 9;", "runtime_error"),
        Case("null-literal", "return {[null]=7};", "runtime_error"),
        Case("missing-set", "local t={};\nt.a=2; return 9;", "runtime_error"),
        Case("missing-get", "local t={};\nreturn t.a;", "runtime_error"),
        Case("invalid-literal", "return {a:1};", "compile_error"),
        Case("invalid-key", 'return {"a"=1};', "compile_error"),
        Case("constant-key", "const a=4;\nlocal t={a=9};\nreturn t.a;"),
        Case(
            "empty-foreach", "local t={};\nlocal n=0; foreach(k,v in t) n++; return n;"
        ),
        Case(
            "one-variable",
            'local t={a=1,b=2,c=3};\nlocal s=""; foreach(v in t) s=s+v; return s;',
        ),
        Case(
            "mutate-value",
            'local t={a=1,b=2,c=3};\nlocal s=""; '
            + "foreach(k,v in t){t[k]=v+10; s=s+k+v;} return s+t.a+t.b+t.c;",
        ),
        Case(
            "rebind-iterator",
            'local t={a=1,b=2,c=3};\nlocal s=""; '
            + "foreach(k,v in t){s=s+k+v; t={x=9};\n} return s;",
        ),
        Case(
            "break",
            'local t={a=1,b=2,c=3};\nlocal s=""; '
            + "foreach(k,v in t){s=s+k; break;} return s;",
        ),
        Case(
            "continue",
            'local t={a=1,b=2,c=3};\nlocal s=""; '
            + "foreach(k,v in t){if(v==2)continue; s=s+k;} return s;",
        ),
        Case(
            "grow-iteration",
            'local t={[0]=0,[4]=4,[8]=8};\nlocal n=0; local s=""; '
            + 'foreach(k,v in t){n++; if(n<12)t[n*16]<-n; s=s+k+","; if(n==20)break;} '
            + "return s;",
        ),
    ]
    for count in [1, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 63, 64, 129]:
        fields = ",".join(f"[{index * 64}]={index}" for index in range(count))
        source = (
            f"local t={{{fields}}};\n"
            + 'local s=""; foreach(k,v in t)s=s+k+":"+v+","; return s;'
        )
        result.append(Case(f"integer-collisions-{count}", source))
        fields = ",".join(f"key{index}={index}" for index in range(count))
        source = (
            f"local t={{{fields}}};\n"
            + 'local s=""; foreach(k,v in t)s=s+k+":"+v+","; return s;'
        )
        result.append(Case(f"string-collisions-{count}", source))
    for length in [
        0,
        1,
        3,
        4,
        7,
        8,
        9,
        15,
        16,
        17,
        31,
        32,
        33,
        63,
        64,
        65,
        127,
        128,
        129,
        255,
        256,
        257,
        1024,
    ]:
        fields = ",".join(f"[{literal(length, seed)}]={seed}" for seed in range(8))
        source = (
            f"local t={{{fields}}};\n"
            + 'local s=""; foreach(k,v in t)s=s+k+":"+v+","; return s;'
        )
        result.append(Case(f"byte-keys-{length}", source))
    result.extend(object_cases())
    return result

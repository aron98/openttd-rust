"""Native delete/clone expressions and physical table mutation cases."""

# Explicit joins keep generated source fragments distinct.
# ruff: noqa: ISC003
from .case import Case, literal


def object_cases() -> list[Case]:
    result = [
        Case(
            "clone-table",
            "local t={a=7,b=9};\nlocal c=clone t; local x=delete t.a; return x+c.a;",
        ),
        Case("clone-distinct", "local t={a=7};\nreturn t==clone t;"),
        Case(
            "clone-array",
            "local t=[7,9]; local c=clone t; c[0]=3; return t[0]*10+c[0];",
        ),
        Case("clone-array-distinct", "local t=[]; return t==clone t;"),
        Case(
            "clone-array-string",
            'local t=["a",null,true,1.5]; local c=clone t; return c[0]+c[1]+c[2]+c[3];',
        ),
        Case("clone-array-temporary", "return (clone [1,2,3])[1];"),
        Case("clone-table-temporary", "return (clone {a=8}).a;"),
        Case(
            "clone-empty",
            "local t=clone {};\nlocal n=0; foreach(k,v in t)n++; return n;",
        ),
        Case("delete-temporary", "return delete {a=8}.a;"),
        Case("delete-computed", "local t={[1]=8};\nlocal k=0; return delete t[++k];"),
        Case("delete-bracket", 'local t={a=8};\nreturn delete t["a"];'),
        Case("delete-root", "held<-7; return delete held;"),
        Case("delete-explicit-root", "held<-7; return delete ::held;"),
        Case(
            "delete-root-object", "held<-{key=8};\nlocal t=delete held; return t.key;"
        ),
        Case("delete-intermediate", "held<-{key=8};\nreturn delete held.key;"),
        Case(
            "delete-intermediate-explicit", "held<-{key=8};\nreturn delete ::held.key;"
        ),
        Case(
            "delete-null",
            "local t={};\nt[null]=7; return delete t[null];",
            "runtime_error",
        ),
        Case("delete-missing", "local t={a=1};\nreturn delete t.b;", "runtime_error"),
        Case(
            "delete-twice",
            "local t={a=1};\ndelete t.a; return delete t.a;",
            "runtime_error",
        ),
        Case(
            "delete-default-method",
            "local t={};\nreturn delete t.len;",
            "runtime_error",
        ),
        Case("delete-local", "local x=7; return delete x;", "compile_error"),
        Case(
            "delete-parenthesized",
            "local t={a=7};\nreturn delete (t.a);",
            "compile_error",
        ),
        Case("delete-literal", "return delete 7;", "compile_error"),
        Case("delete-constant", "const x=7; return delete x;", "compile_error"),
        Case(
            "clone-alias-expression",
            "local t=[1]; local c=clone t; return (c[0]=9)+t[0];",
        ),
    ]
    result.extend(
        [
            Case(
                "adjacent-unary",
                'local a=7; return (-a)+(~a)+(!a?1:0)+(typeof a=="integer"?1:0);',
            ),
            Case(
                "adjacent-array-mutation",
                "local a=[1,2,3]; local s=0; "
                + "foreach(k,v in a){a[k]=v+2;s+=a[k];} return s;",
            ),
            Case(
                "adjacent-string-foreach",
                'local s=""; foreach(k,v in "a\\x00\\xFFz")s=s+k+":"+v+","; return s;',
            ),
            Case("adjacent-root-unary", "value<-7; value=-value; return ~value;"),
            Case(
                "adjacent-array-index-float", "local a=[3,5]; a[1.9]=9; return a[1.0];"
            ),
            Case(
                "adjacent-string-rebind",
                'local t="abc"; local s=""; foreach(k,v in t){t="z";s=s+v;} return s;',
            ),
            Case(
                "delete-parenthesized-receiver", "local t={a=7};\nreturn delete (t).a;"
            ),
            Case(
                "clone-parenthesized", "local t={a=7};\nlocal c=clone (t); return c.a;"
            ),
            Case(
                "delete-assigned-key",
                "local t={[1]=7};\nlocal x=0; return delete t[(x=1)];",
            ),
            Case("delete-binary", "local t={a=7,b=9};\nreturn delete t.a+delete t.b;"),
            Case(
                "delete-reinsert",
                "local t={a=7};\nlocal x=delete t.a; t.a<-9; return x+t.a;",
            ),
            Case(
                "clone-null-hidden",
                'local t={};\nt[null]="hidden"; local c=clone t; '
                + "local n=0; foreach(k,v in c)n++; return n;",
            ),
            Case("clone-nested", "local t={a=7};\nreturn (clone clone t).a;"),
            Case("clone-unary-array", "return !(clone []);"),
            Case(
                "clone-field-error",
                "local t={a=7};\nreturn clone t.a;",
                "runtime_error",
            ),
            Case("clone-expression-identity", "local t=[1]; return clone t==t;"),
        ]
    )
    for length in [0, 1, 8, 17, 33, 65, 256]:
        fields = ",".join(f"[{literal(length, seed)}]={seed}" for seed in range(8))
        source = f"local t={{{fields}}};\nlocal c=clone t; "
        source += (
            'local s=""; foreach(k,v in c){s=s+k+":"+(delete t[k])+",";} return s;'
        )
        result.append(Case(f"clone-delete-bytes-{length}", source))
    for key in ["true", "false", "1.0", "-0.0", "-1", "1"]:
        source = (
            f"local t={{[{key}]=9}};\nlocal c=clone t; "
            + f"local x=delete t[{key}]; return x+c[{key}];"
        )
        result.append(Case("delete-scalar-" + key, source))
    for index, scalar in enumerate(["null", "true", "7", "1.5", '"text"']):
        result.append(
            Case(f"clone-type-{index}", f"return clone {scalar};", "runtime_error")
        )
        result.append(
            Case(
                f"delete-type-{index}",
                f"local x={scalar}; return delete x.a;",
                "runtime_error",
            )
        )
    result.append(
        Case("delete-array-type", "local x=[1]; return delete x[0];", "runtime_error")
    )
    for size in [1, 3, 4, 7, 8, 15, 16, 33]:
        for stride in [1, 4, 32]:
            setup = f"local t={{}};\nfor(local i=0;i<{size};i++)t[i*{stride}]<-i; "
            result.append(
                Case(
                    f"delete-chain-{size}-{stride}",
                    setup
                    + f"for(local i=0;i<{size};i+=2)delete t[i*{stride}]; "
                    + 'local c=clone t; local s=""; '
                    + 'foreach(k,v in c)s=s+k+":"+v+","; return s;',
                )
            )
            result.append(
                Case(
                    f"delete-reuse-{size}-{stride}",
                    setup
                    + f"for(local i=0;i<{size};i+=2)delete t[i*{stride}]; "
                    + f"for(local i=0;i<{size};i++)t[(i+{size})*{stride}]<-i+100; "
                    + 'local c=clone t; local s=""; '
                    + 'foreach(k,v in t)s=s+k+":"+v+","; s=s+"/"; '
                    + 'foreach(k,v in c)s=s+k+":"+v+","; return s;',
                )
            )
            result.append(
                Case(
                    f"delete-live-{size}-{stride}",
                    setup
                    + 'local s=""; '
                    + 'foreach(k,v in t){s=s+k+":"+v+","; delete t[k];} return s;',
                )
            )
            result.append(
                Case(
                    f"clone-live-{size}-{stride}",
                    setup
                    + 'local c=clone t; local s=""; '
                    + 'foreach(k,v in c){delete t[k];s=s+k+":"+v+",";} return s;',
                )
            )
    return result

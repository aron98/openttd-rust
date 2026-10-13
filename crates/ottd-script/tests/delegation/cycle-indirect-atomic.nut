local a={};
 local b={};
 local c={};
 delegate a:b;
 delegate b:c;
 local caught=false;
 try{delegate c:a;
}catch(e){caught=true;
 assert(e=="delegate cycle detected");
} assert(caught);
 assert(a.parent==null);
 assert(c.parent==b);
 return 1;


local d={x=7};
 local t={};
 delegate d:t;
 local caught=false;
 try{delegate t:t;
}catch(e){caught=true;
 assert(e=="delegate cycle detected");
} assert(caught);
 assert(t.parent==d);
 return 1;


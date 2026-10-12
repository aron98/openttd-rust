local d={x=7};
 local t={};
 delegate d:t;
 local caught=false;
 try{delegate 2:t;
}catch(e){caught=true;
 assert(e=="using 'integer' as delegate");
} assert(caught);
 assert(t.parent==d);
 return 1;


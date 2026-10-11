local d={x=7};
 local t={};
 local alias=t;
 delegate d:t;
 delegate null:alias;
 assert(t.parent==null);
 assert(! ("x" in t));
 return 1;


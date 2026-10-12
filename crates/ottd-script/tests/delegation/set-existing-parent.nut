local d={x=7};
 local t={};
 delegate d:t;
 t.x=11;
 assert(d.x==11);
 delegate null:t;
 assert(! ("x" in t));
 return 1;


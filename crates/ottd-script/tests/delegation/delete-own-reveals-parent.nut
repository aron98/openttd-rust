local d={x=7};
 local t={x=2};
 delegate d:t;
 assert((delete t.x)==2);
 assert(t.x==7);
 return 1;


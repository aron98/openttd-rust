local a={x=1};
 local b={x=2};
 local c={};
 delegate a:b;
 delegate b:c;
 c.x=3;
 assert(b.x==3);
 assert(a.x==1);
 return 1;


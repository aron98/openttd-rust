local a={x=9};
 local b={};
 local c={};
 delegate a:b;
 delegate b:c;
 assert(c.x==9);
 assert(c.parent==b);
 assert(c.parent.parent==a);
 return 1;


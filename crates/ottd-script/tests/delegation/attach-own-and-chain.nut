local d={x=7};
 local t={y=2};
 local a=delegate d:t;
 assert(a==t);
 assert(t.x==7);
 assert(t.y==2);
 return 1;


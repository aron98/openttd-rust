local d={x=7};
 local t={x=2};
 delegate d:t;
 assert(t.x==2);
 return 1;


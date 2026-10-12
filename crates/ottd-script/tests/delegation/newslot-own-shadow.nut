local d={x=7};
 local t={};
 delegate d:t;
 t.x<-8;
 assert(t.x==8);
 assert(d.x==7);
 return 1;


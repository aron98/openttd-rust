local d={x=7};
 local t={};
 delegate d:t;
 d.owner<-t;
 assert(t.parent.owner==t);
 d=null;
 assert(t.x==7);
 return 1;


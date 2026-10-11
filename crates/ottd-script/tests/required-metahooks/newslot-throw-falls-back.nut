events<-{n=0};
 local d={_newslot=function(k,v){events.n=v;
 throw "newslot";
}};
 local t={};
 delegate d:t;
 t.x<-9;
 assert(t.x==9);
 assert(events.n==9);
 return 1;


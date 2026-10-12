events<-{n=0};
 local d={_set=function(k,v){events.n=v;
}};
 local t={};
 delegate d:t;
 t.x=9;
 assert(events.n==9);
 assert(! ("x" in t));
 return 1;


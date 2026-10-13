events<-{n=0};
 local d={_get=function(k){events.n+=1;
 return 7;
}};
 local t={};
 delegate d:t;
 assert(! ("x" in t));
 assert(events.n==0);
 return 1;


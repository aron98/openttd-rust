events<-{n=0};
 local d={_get=function(k){::events.n+=1;
 throw "hook";
}};
 local t={};
 delegate d:t;
 local caught=false;
 try{local x=t.x;
}catch(e){caught=true;
 assert(e=="the index 'x' does not exist");
} assert(caught);
 assert(events.n==1);
 return 1;


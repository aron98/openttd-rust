local d={_newslot=function(k,v){throw "bad";}};
local t=delegate d:{x=1};
t.x<-9;
assert(t.x==9);
return 1;
